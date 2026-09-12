//! TransferExecutor — pure VFS streaming owned by transfer layer.
//! No dependency on AppState / Axum / HTTP. Caller (application) adapts
//! `axum::extract::multipart::Field` → `Stream<Item=Result<Bytes, AppError>>`
//! and handles lock ordering + job creation before calling here.

use crate::domain::VfsPath;
use crate::errors::{AppError, VfsError};
use crate::transfer::plan::TransferPlan;
use crate::transfer::TransferManager;
use crate::vfs::FileSystem;
use bytes::Bytes;
use futures::Stream;
use futures::StreamExt;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::AsyncWriteExt;

/// Inputs which describe one inline upload. Keeping these together prevents
/// positional-argument mixups at the application/transfer boundary.
#[derive(Debug, Clone)]
pub struct InlineUploadContext {
    pub target: VfsPath,
    pub job_id: String,
    pub plan: TransferPlan,
    pub total_hint: Option<u64>,
    pub max_bytes: u64,
    pub target_exists: bool,
    pub target_perms: Option<String>,
}

async fn cleanup_upload_target(provider: &dyn FileSystem, path: &VfsPath, job_id: &str) {
    if let Err(error) = provider.delete(path).await {
        tracing::warn!(job_id, path = %path.path, ?error, "failed to clean up upload artifact");
    }
}

/// Pure executor for inline upload streaming.
/// Owns: duplex, write_stream, progress, cancellation, staging commit, cleanup.
/// Does NOT own: validation, lock acquisition, job creation, cache invalidation, audit.
/// Those stay in UploadApplicationService wiring layer (explicit dependencies).
pub async fn execute_inline_upload_stream<S>(
    manager: &TransferManager,
    provider: Arc<dyn FileSystem>,
    context: InlineUploadContext,
    cancel_token: tokio_util::sync::CancellationToken,
    byte_stream: S,
) -> Result<u64, AppError>
where
    S: Stream<Item = Result<Bytes, AppError>> + Send,
{
    if cancel_token.is_cancelled() {
        if context.plan.uses_staging() {
            if let Some(staging) = context.plan.staging_path(&context.target, &context.job_id) {
                cleanup_upload_target(provider.as_ref(), &staging, &context.job_id).await;
            }
        }
        return Err(AppError::Cancelled("upload cancelled before start".into()));
    }

    let write_target = context
        .plan
        .staging_path(&context.target, &context.job_id)
        .unwrap_or_else(|| context.target.clone());
    let use_staging = context.plan.uses_staging();

    let (duplex_reader, mut duplex_writer) = tokio::io::duplex(64 * 1024);

    let write_handle = tokio::spawn({
        let provider = provider.clone();
        let write_target = write_target.clone();
        let cancel_token = cancel_token.clone();
        async move {
            tokio::select! {
                res = provider.write_stream(&write_target, Box::new(duplex_reader)) => res,
                _ = cancel_token.cancelled() => Err(VfsError::IoError("Upload cancelled by client".into())),
            }
        }
    });

    let mut uploaded_bytes: u64 = 0;
    let mut stream_err: Option<AppError> = None;
    let now = Instant::now();
    let mut rate_estimator = crate::transfer::rate::TransferRateEstimator::new(now, 0);
    let mut last_emit = now;
    let total_for_progress = context.total_hint.unwrap_or(0);

    futures::pin_mut!(byte_stream);
    loop {
        if cancel_token.is_cancelled() {
            stream_err = Some(AppError::Cancelled("upload cancelled".into()));
            break;
        }
        let item_opt = tokio::select! {
            _ = cancel_token.cancelled() => {
                stream_err = Some(AppError::Cancelled("upload cancelled".into()));
                break;
            }
            item = byte_stream.next() => item,
        };
        let Some(item) = item_opt else { break };
        let chunk = match item {
            Ok(c) => c,
            Err(e) => {
                stream_err = Some(e);
                break;
            }
        };
        uploaded_bytes += chunk.len() as u64;
        if uploaded_bytes > context.max_bytes {
            stream_err = Some(AppError::PayloadTooLarge(format!(
                "Uploaded file exceeded maximum upload size limit of {} bytes",
                context.max_bytes
            )));
            break;
        }
        let write_fut = duplex_writer.write_all(&chunk);
        tokio::select! {
            _ = cancel_token.cancelled() => {
                stream_err = Some(AppError::Cancelled("upload cancelled".into()));
                break;
            }
            res = write_fut => {
                if let Err(e) = res {
                    stream_err = Some(AppError::Internal(anyhow::anyhow!(
                        "Failed writing upload chunk: {}",
                        e
                    )));
                    break;
                }
            }
        }
        let now = Instant::now();
        if now.duration_since(last_emit).as_millis() >= 100 {
            let sample = rate_estimator.observe(now, uploaded_bytes, total_for_progress);
            manager
                .update_inline_progress(
                    &context.job_id,
                    uploaded_bytes,
                    total_for_progress,
                    sample.speed_bytes_per_sec,
                    sample.eta_seconds,
                )
                .await;
            last_emit = now;
        }
    }
    drop(duplex_writer);

    if cancel_token.is_cancelled() {
        let _ = tokio::time::timeout(std::time::Duration::from_millis(200), write_handle).await;
        if use_staging || !context.target_exists {
            cleanup_upload_target(provider.as_ref(), &write_target, &context.job_id).await;
        }
        if let Some(err) = stream_err {
            if matches!(err, AppError::Cancelled(_)) {
                return Err(err);
            }
        }
        return Err(AppError::Cancelled("upload cancelled".into()));
    }

    if let Some(err) = stream_err {
        let _ = write_handle.await;
        if use_staging || !context.target_exists {
            cleanup_upload_target(provider.as_ref(), &write_target, &context.job_id).await;
        }
        return Err(err);
    }

    let write_res = write_handle
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Upload worker task error: {}", e)))?;

    if let Err(e) = write_res {
        if use_staging || !context.target_exists {
            cleanup_upload_target(provider.as_ref(), &write_target, &context.job_id).await;
        }
        return Err(AppError::from(e));
    }

    let can_commit = manager
        .try_enter_finalizing(&context.job_id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("try_enter_finalizing failed: {}", e)))?;
    if !can_commit {
        if use_staging {
            cleanup_upload_target(provider.as_ref(), &write_target, &context.job_id).await;
        } else if !context.target_exists {
            cleanup_upload_target(provider.as_ref(), &context.target, &context.job_id).await;
        }
        return Err(AppError::Cancelled("upload cancelled before commit".into()));
    }

    if use_staging {
        if let Some(ref perms) = context.target_perms {
            if let Err(error) = provider.set_permissions(&write_target, perms).await {
                cleanup_upload_target(provider.as_ref(), &write_target, &context.job_id).await;
                return Err(AppError::from(error));
            }
        }

        if let Err(rename_err) = provider.rename(&write_target, &context.target).await {
            cleanup_upload_target(provider.as_ref(), &write_target, &context.job_id).await;
            return Err(AppError::Internal(anyhow::anyhow!(format!(
                "Failed to promote staging file to final destination '{}': {}",
                context.target.path, rename_err
            ))));
        }
    } else if let Some(ref perms) = context.target_perms {
        if let Err(error) = provider.set_permissions(&context.target, perms).await {
            return Err(AppError::Internal(anyhow::anyhow!(
                "Upload content for '{}' was written, but applying inherited permissions '{}' failed: {}. Filesystem mutation committed; recovery required",
                context.target.path,
                perms,
                error
            )));
        }
    }

    let final_total = if total_for_progress > 0 {
        total_for_progress
    } else {
        uploaded_bytes
    };

    manager
        .update_inline_progress(&context.job_id, uploaded_bytes, final_total, 0, Some(0))
        .await;

    Ok(uploaded_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db::init_db,
        domain::{Capabilities, FileEntry, FileMetadata, VfsPath},
        errors::VfsError,
        events::EventJournal,
        transfer::{TransferPlan, TransferStaging},
        vfs::{registry::ProviderRegistry, FileSystem},
    };
    use bytes::Bytes;
    use futures::Stream;
    use std::{
        pin::Pin,
        sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            Arc,
        },
    };
    use tokio::sync::Notify;
    use tokio_util::{sync::CancellationToken, task::TaskTracker};

    struct ManagerFixture {
        manager: TransferManager,
        _temp: tempfile::TempDir,
        shutdown: CancellationToken,
        tracker: TaskTracker,
    }

    impl Drop for ManagerFixture {
        fn drop(&mut self) {
            self.shutdown.cancel();
            self.tracker.close();
        }
    }

    struct BlockingMockFs {
        capabilities: Capabilities,
        write_started: Arc<Notify>,
        delete_called: Arc<AtomicBool>,
        rename_called: Arc<AtomicBool>,
        rename_started: Arc<Notify>,
        rename_continue: Arc<Notify>,
        write_calls: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl FileSystem for BlockingMockFs {
        fn capabilities(&self) -> Capabilities {
            self.capabilities.clone()
        }
        async fn list_stream(
            &self,
            _path: &VfsPath,
        ) -> Result<
            Pin<Box<dyn Stream<Item = Result<FileEntry, VfsError>> + Send + 'static>>,
            VfsError,
        > {
            Err(VfsError::IoError("not implemented".into()))
        }
        async fn stat(&self, _path: &VfsPath) -> Result<FileMetadata, VfsError> {
            Err(VfsError::IoError("not found".into()))
        }
        async fn read_stream(
            &self,
            _path: &VfsPath,
        ) -> Result<Box<dyn tokio::io::AsyncRead + Send + Unpin>, VfsError> {
            Err(VfsError::IoError("not implemented".into()))
        }
        async fn read_range(
            &self,
            _path: &VfsPath,
            _offset: u64,
            _length: u64,
        ) -> Result<Box<dyn tokio::io::AsyncRead + Send + Unpin>, VfsError> {
            Err(VfsError::IoError("not implemented".into()))
        }
        async fn write_stream(
            &self,
            _path: &VfsPath,
            mut input: Box<dyn tokio::io::AsyncRead + Send + Unpin>,
        ) -> Result<(), VfsError> {
            self.write_calls.fetch_add(1, Ordering::SeqCst);
            self.write_started.notify_one();
            let mut buf = vec![0u8; 8192];
            use tokio::io::AsyncReadExt;
            loop {
                match input.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(_) => continue,
                    Err(e) => return Err(VfsError::IoError(e.to_string())),
                }
            }
            Ok(())
        }
        async fn create_file(&self, _path: &VfsPath) -> Result<(), VfsError> {
            Ok(())
        }
        async fn create_dir(&self, _path: &VfsPath) -> Result<(), VfsError> {
            Ok(())
        }
        async fn delete(&self, _path: &VfsPath) -> Result<(), VfsError> {
            self.delete_called.store(true, Ordering::SeqCst);
            Ok(())
        }
        async fn rename(&self, _from: &VfsPath, _to: &VfsPath) -> Result<(), VfsError> {
            self.rename_started.notify_one();
            self.rename_continue.notified().await;
            self.rename_called.store(true, Ordering::SeqCst);
            Ok(())
        }
        async fn copy(&self, _from: &VfsPath, _to: &VfsPath) -> Result<(), VfsError> {
            Ok(())
        }
    }

    async fn setup_manager() -> (ManagerFixture, Arc<BlockingMockFs>) {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("p0_test.db");
        let database_url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
        let db = init_db(&database_url).await.unwrap();
        let journal = Arc::new(EventJournal::init(db.clone()).await.unwrap());
        let registry = ProviderRegistry::new();
        let shutdown = CancellationToken::new();
        let tracker = TaskTracker::new();
        let manager = TransferManager::new(
            registry.providers_map(),
            db,
            4,
            journal,
            shutdown.clone(),
            &tracker,
        )
        .await;
        let caps = Capabilities {
            atomic_rename: true,
            write: true,
            read: true,
            ..Default::default()
        };
        let mock = Arc::new(BlockingMockFs {
            capabilities: caps,
            write_started: Arc::new(Notify::new()),
            delete_called: Arc::new(AtomicBool::new(false)),
            rename_called: Arc::new(AtomicBool::new(false)),
            rename_started: Arc::new(Notify::new()),
            rename_continue: Arc::new(Notify::new()),
            write_calls: Arc::new(AtomicUsize::new(0)),
        });
        (
            ManagerFixture {
                manager,
                _temp: temp,
                shutdown,
                tracker,
            },
            mock,
        )
    }

    fn plan() -> TransferPlan {
        TransferPlan {
            execution_mode: crate::transfer::TransferExecutionMode::Inline,
            staging: TransferStaging::LocalTemp,
            commit: crate::domain::CommitSemantics::AtomicRename,
        }
    }

    #[tokio::test]
    async fn test_initial_cancel_no_commit() {
        let (fixture, mock) = setup_manager().await;
        let provider: Arc<dyn FileSystem> = mock.clone();
        let target = VfsPath::new("local", "/upload_test.txt").unwrap();
        let plan = plan();
        let job = fixture
            .manager
            .create_inline_upload_job_with_plan(
                Some("user1".into()),
                "upload_test.txt".into(),
                "local".into(),
                "/upload_test.txt".into(),
                Some(1024),
                plan.clone(),
            )
            .await;
        let token = fixture.manager.cancel_token(&job.id).expect("token exists");
        token.cancel();
        let _ = fixture
            .manager
            .cancel_job(&job.id, Some("user1"), false)
            .await;
        let byte_stream = futures::stream::empty::<Result<Bytes, AppError>>();
        let res = execute_inline_upload_stream(
            &fixture.manager,
            provider,
            InlineUploadContext {
                target,
                job_id: job.id.clone(),
                plan,
                total_hint: Some(1024),
                max_bytes: 10 * 1024 * 1024,
                target_exists: false,
                target_perms: None,
            },
            token,
            byte_stream,
        )
        .await;
        assert!(matches!(res, Err(AppError::Cancelled(_))));
        assert!(!mock.rename_called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_mid_stream_cancel_aborts_and_no_rename() {
        let (fixture, mock) = setup_manager().await;
        let provider: Arc<dyn FileSystem> = mock.clone();
        let target = VfsPath::new("local", "/mid_cancel.txt").unwrap();
        let plan = plan();
        let job = fixture
            .manager
            .create_inline_upload_job_with_plan(
                Some("user1".into()),
                "mid_cancel.txt".into(),
                "local".into(),
                "/mid_cancel.txt".into(),
                Some(1024 * 1024),
                plan.clone(),
            )
            .await;
        let token = fixture.manager.cancel_token(&job.id).expect("token exists");
        let byte_stream = futures::stream::unfold(0, |state| async move {
            if state == 0 {
                Some((Ok(Bytes::from(vec![1u8; 1024])), 1))
            } else {
                std::future::pending::<Option<(Result<Bytes, AppError>, i32)>>().await
            }
        });
        let manager_clone = fixture.manager.clone();
        let job_id_clone = job.id.clone();
        let token_clone = token.clone();
        let provider_clone = provider.clone();
        let plan_clone = plan.clone();
        let target_clone = target.clone();
        let handle = tokio::spawn(async move {
            execute_inline_upload_stream(
                &manager_clone,
                provider_clone,
                InlineUploadContext {
                    target: target_clone,
                    job_id: job_id_clone,
                    plan: plan_clone,
                    total_hint: Some(1024 * 1024),
                    max_bytes: 10 * 1024 * 1024,
                    target_exists: false,
                    target_perms: None,
                },
                token_clone,
                byte_stream,
            )
            .await
        });
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            mock.write_started.notified(),
        )
        .await
        .expect("write_stream should start");
        let _ = fixture
            .manager
            .cancel_job(&job.id, Some("user1"), false)
            .await;
        assert!(token.is_cancelled());
        let res = tokio::time::timeout(std::time::Duration::from_secs(3), handle)
            .await
            .expect("executor should finish after cancel")
            .unwrap();
        assert!(matches!(res, Err(AppError::Cancelled(_))));
        assert!(!mock.rename_called.load(Ordering::SeqCst));
        assert!(mock.write_calls.load(Ordering::SeqCst) >= 1);
    }

    #[tokio::test]
    async fn test_cancel_during_rename_is_too_late() {
        let (fixture, mock) = setup_manager().await;
        let provider: Arc<dyn FileSystem> = mock.clone();
        let target = VfsPath::new("local", "/race_rename.txt").unwrap();
        let plan = plan();
        let job = fixture
            .manager
            .create_inline_upload_job_with_plan(
                Some("user1".into()),
                "race_rename.txt".into(),
                "local".into(),
                "/race_rename.txt".into(),
                Some(10),
                plan.clone(),
            )
            .await;
        let token = fixture.manager.cancel_token(&job.id).expect("token exists");
        let byte_stream = futures::stream::once(async { Ok(Bytes::from(vec![1u8; 10])) });
        let manager_clone = fixture.manager.clone();
        let job_id_clone = job.id.clone();
        let token_clone = token.clone();
        let provider_clone = provider.clone();
        let plan_clone = plan.clone();
        let target_clone = target.clone();
        let handle = tokio::spawn(async move {
            execute_inline_upload_stream(
                &manager_clone,
                provider_clone,
                InlineUploadContext {
                    target: target_clone,
                    job_id: job_id_clone,
                    plan: plan_clone,
                    total_hint: Some(10),
                    max_bytes: 10 * 1024 * 1024,
                    target_exists: false,
                    target_perms: None,
                },
                token_clone,
                byte_stream,
            )
            .await
        });
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            mock.rename_started.notified(),
        )
        .await
        .expect("rename should start");
        let cancel_res = fixture
            .manager
            .cancel_job(&job.id, Some("user1"), false)
            .await;
        assert!(matches!(
            cancel_res,
            Err(crate::transfer::CancelTransferError::NotCancellable(_))
        ));
        mock.rename_continue.notify_waiters();
        let res = tokio::time::timeout(std::time::Duration::from_secs(3), handle)
            .await
            .expect("executor should complete")
            .unwrap();
        assert!(res.is_ok());
        assert!(mock.rename_called.load(Ordering::SeqCst));
    }
}
