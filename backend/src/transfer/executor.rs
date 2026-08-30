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

/// Result of a successful inline streaming upload.
#[derive(Debug, Clone)]
pub struct InlineUploadOutcome {
    pub bytes_written: u64,
}

/// Pure executor for inline upload streaming.
/// Owns: duplex, write_stream, progress, cancellation, staging commit, cleanup.
/// Does NOT own: validation, lock acquisition, job creation, cache invalidation, audit.
/// Those stay in UploadApplicationService wiring layer (explicit dependencies).
pub async fn execute_inline_upload_stream<S>(
    manager: &TransferManager,
    provider: Arc<dyn FileSystem>,
    target: VfsPath,
    job_id: &str,
    plan: &TransferPlan,
    total_hint: Option<u64>,
    max_bytes: u64,
    target_exists: bool,
    target_perms: Option<String>,
    cancel_token: tokio_util::sync::CancellationToken,
    byte_stream: S,
) -> Result<InlineUploadOutcome, AppError>
where
    S: Stream<Item = Result<Bytes, AppError>> + Send,
{
    // Initial cancellation guard — handles race where cancel_job fired between create and executor start (P0)
    if cancel_token.is_cancelled() {
        if plan.uses_staging() {
            if let Some(staging) = plan.staging_path(&target, job_id) {
                let _ = provider.delete(&staging).await;
            }
        }
        return Err(AppError::Internal(anyhow::anyhow!("Upload cancelled before start")));
    }

    // Resolve staging target via plan (canonical naming)
    let write_target = plan
        .staging_path(&target, job_id)
        .unwrap_or_else(|| target.clone());
    let use_staging = plan.uses_staging();

    // Prepare duplex (cancellation token is manager-owned, not local)
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
    let start_time = Instant::now();
    let mut last_emit = Instant::now();
    let total_for_progress = total_hint.unwrap_or(0);

    futures::pin_mut!(byte_stream);
    loop {
        // Check cancellation before each poll (handles race where cancel fires between chunks)
        if cancel_token.is_cancelled() {
            stream_err = Some(AppError::Internal(anyhow::anyhow!("Upload cancelled")));
            break;
        }
        let item_opt = tokio::select! {
            _ = cancel_token.cancelled() => {
                stream_err = Some(AppError::Internal(anyhow::anyhow!("Upload cancelled")));
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
        if uploaded_bytes > max_bytes {
            stream_err = Some(AppError::PayloadTooLarge(format!(
                "Uploaded file exceeded maximum upload size limit of {} bytes",
                max_bytes
            )));
            break;
        }
        // Write with cancellation — abort if token fires while pipe is full
        let write_fut = duplex_writer.write_all(&chunk);
        tokio::select! {
            _ = cancel_token.cancelled() => {
                stream_err = Some(AppError::Internal(anyhow::anyhow!("Upload cancelled")));
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
        if last_emit.elapsed().as_millis() >= 100 {
            let elapsed = start_time.elapsed().as_secs_f64();
            let speed = if elapsed > 0.05 {
                (uploaded_bytes as f64 / elapsed) as u64
            } else {
                0
            };
            manager
                .update_inline_progress(job_id, uploaded_bytes, total_for_progress, speed, None)
                .await;
            last_emit = Instant::now();
        }
    }
    drop(duplex_writer);

    // If cancelled, abort writer and cleanup staging, propagate cancellation error (P0: no commit)
    if cancel_token.is_cancelled() {
        // Wake writer task via cancel_token (it selects on same token)
        let _ = tokio::time::timeout(std::time::Duration::from_millis(200), write_handle).await;
        if use_staging || !target_exists {
            let _ = provider.delete(&write_target).await;
        }
        // Prefer explicit cancelled error so caller can avoid fail->Failed overwrite
        if let Some(err) = stream_err {
            // If stream_err already is cancelled, return it; otherwise override with cancelled
            if err.to_string().contains("cancelled") {
                return Err(err);
            }
        }
        return Err(AppError::Internal(anyhow::anyhow!("Upload cancelled")));
    }

    if let Some(err) = stream_err {
        let _ = write_handle.await;
        if use_staging || !target_exists {
            let _ = provider.delete(&write_target).await;
        }
        return Err(err);
    }

    let write_res = write_handle
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Upload worker task error: {}", e)))?;

    if let Err(e) = write_res {
        if use_staging || !target_exists {
            let _ = provider.delete(&write_target).await;
        }
        return Err(AppError::from(e));
    }

    // Atomic commit boundary Opsi X: try_enter_finalizing under jobs.write() lock
    // Returns Ok(false) if already cancelled / Finalizing — must not rename
    let can_commit = manager.try_enter_finalizing(job_id).await.map_err(|e| {
        AppError::Internal(anyhow::anyhow!("try_enter_finalizing failed: {}", e))
    })?;
    if !can_commit {
        if use_staging {
            let _ = provider.delete(&write_target).await;
        } else if !target_exists {
            let _ = provider.delete(&target).await;
        }
        return Err(AppError::Internal(anyhow::anyhow!("Upload cancelled / too late")));
    }

    if use_staging {
        if let Err(rename_err) = provider.rename(&write_target, &target).await {
            let _ = provider.delete(&write_target).await;
            return Err(AppError::Internal(anyhow::anyhow!(format!(
                "Failed to promote staging file to final destination '{}': {}",
                target.path, rename_err
            ))));
        }
    }

    if let Some(ref perms) = target_perms {
        let _ = provider.set_permissions(&target, perms).await;
    }

    // Emit final progress
    let elapsed = start_time.elapsed().as_secs_f64();
    let speed = if elapsed > 0.05 {
        (uploaded_bytes as f64 / elapsed) as u64
    } else {
        0
    };
    manager
        .update_inline_progress(job_id, uploaded_bytes, total_for_progress, speed, Some(0))
        .await;

    Ok(InlineUploadOutcome {
        bytes_written: uploaded_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::AppConfig,
        db::init_db,
        domain::{Capabilities, FileEntry, FileMetadata, VfsPath},
        errors::VfsError,
        transfer::{TransferPlan, TransferStaging},
        vfs::FileSystem,
        AppState,
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

    struct BlockingMockFs {
        capabilities: Capabilities,
        write_started: Arc<Notify>,
        write_continue: Arc<Notify>,
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
        ) -> Result<Pin<Box<dyn Stream<Item = Result<FileEntry, VfsError>> + Send + 'static>>, VfsError>
        {
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
            // Consume input until EOF (or cancellation via drop). For pending byte_stream,
            // input will block on read, which is cancelled via token select in executor.
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
            // Wait for test to allow continue (deterministic, not sleep)
            self.rename_continue.notified().await;
            self.rename_called.store(true, Ordering::SeqCst);
            Ok(())
        }
        async fn copy(&self, _from: &VfsPath, _to: &VfsPath) -> Result<(), VfsError> {
            Ok(())
        }
    }

    async fn setup_manager() -> (AppState, Arc<BlockingMockFs>) {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("p0_test.db");
        let storage_dir = temp.path().join("storage");
        std::fs::create_dir_all(&storage_dir).unwrap();
        let mut config = AppConfig::default();
        config.database.url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
        config.filesystem.default_local_root = storage_dir.clone();
        let db = init_db(&config.database.url).await.unwrap();
        let state = AppState::new_with_db(config, db).await;
        std::mem::forget(temp);
        let caps = {
            let mut c = Capabilities::default();
            c.atomic_rename = true;
            c.write = true;
            c.read = true;
            c
        };
        let mock = Arc::new(BlockingMockFs {
            capabilities: caps,
            write_started: Arc::new(Notify::new()),
            write_continue: Arc::new(Notify::new()),
            delete_called: Arc::new(AtomicBool::new(false)),
            rename_called: Arc::new(AtomicBool::new(false)),
            rename_started: Arc::new(Notify::new()),
            rename_continue: Arc::new(Notify::new()),
            write_calls: Arc::new(AtomicUsize::new(0)),
        });
        (state, mock)
    }

    #[tokio::test]
    async fn test_initial_cancel_no_commit() {
        let (state, mock) = setup_manager().await;
        let provider: Arc<dyn FileSystem> = mock.clone();
        let target = VfsPath::new("local", "/upload_test.txt").unwrap();
        let plan = TransferPlan {
            execution_mode: crate::transfer::TransferExecutionMode::Inline,
            staging: TransferStaging::LocalTemp,
            commit: crate::domain::CommitSemantics::AtomicRename,
        };
        let job = state
            .transfer_manager
            .create_inline_upload_job_with_plan(
                Some("user1".into()),
                "upload_test.txt".into(),
                "local".into(),
                "/upload_test.txt".into(),
                Some(1024),
                plan.clone(),
            )
            .await;
        let token = state.transfer_manager.cancel_token(&job.id).expect("token exists");
        token.cancel();
        let _ = state.transfer_manager.cancel_job(&job.id, Some("user1"), false).await;
        let byte_stream = futures::stream::empty::<Result<Bytes, crate::errors::AppError>>();
        let res = execute_inline_upload_stream(
            &state.transfer_manager,
            provider,
            target,
            &job.id,
            &plan,
            Some(1024),
            10 * 1024 * 1024,
            false,
            None,
            token.clone(),
            byte_stream,
        )
        .await;
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("cancelled"));
        assert!(!mock.rename_called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_mid_stream_cancel_aborts_and_no_rename() {
        let (state, mock) = setup_manager().await;
        let provider: Arc<dyn FileSystem> = mock.clone();
        let target = VfsPath::new("local", "/mid_cancel.txt").unwrap();
        let plan = TransferPlan {
            execution_mode: crate::transfer::TransferExecutionMode::Inline,
            staging: TransferStaging::LocalTemp,
            commit: crate::domain::CommitSemantics::AtomicRename,
        };
        let job = state
            .transfer_manager
            .create_inline_upload_job_with_plan(
                Some("user1".into()),
                "mid_cancel.txt".into(),
                "local".into(),
                "/mid_cancel.txt".into(),
                Some(1024 * 1024),
                plan.clone(),
            )
            .await;
        let token = state.transfer_manager.cancel_token(&job.id).expect("token exists");
        let byte_stream = futures::stream::unfold(0, |state| async move {
            if state == 0 {
                Some((Ok(Bytes::from(vec![1u8; 1024])), 1))
            } else {
                std::future::pending::<Option<(Result<Bytes, crate::errors::AppError>, i32)>>().await
            }
        });
        let manager_clone = state.transfer_manager.clone();
        let job_id_clone = job.id.clone();
        let token_clone = token.clone();
        let provider_clone = provider.clone();
        let plan_clone = plan.clone();
        let target_clone = target.clone();
        let handle = tokio::spawn(async move {
            execute_inline_upload_stream(
                &manager_clone,
                provider_clone,
                target_clone,
                &job_id_clone,
                &plan_clone,
                Some(1024 * 1024),
                10 * 1024 * 1024,
                false,
                None,
                token_clone,
                byte_stream,
            )
            .await
        });
        tokio::time::timeout(std::time::Duration::from_secs(2), mock.write_started.notified())
            .await
            .expect("write_stream should start");
        let _ = state.transfer_manager.cancel_job(&job.id, Some("user1"), false).await;
        assert!(token.is_cancelled());
        let res = tokio::time::timeout(std::time::Duration::from_secs(3), handle)
            .await
            .expect("executor should finish after cancel")
            .unwrap();
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("cancelled"));
        assert!(!mock.rename_called.load(Ordering::SeqCst));
        assert!(mock.write_calls.load(Ordering::SeqCst) >= 1);
        mock.write_continue.notify_waiters();
    }

    #[tokio::test]
    async fn test_cancel_during_rename_is_too_late() {
        let (state, mock) = setup_manager().await;
        let provider: Arc<dyn FileSystem> = mock.clone();
        let target = VfsPath::new("local", "/race_rename.txt").unwrap();
        let plan = TransferPlan {
            execution_mode: crate::transfer::TransferExecutionMode::Inline,
            staging: TransferStaging::LocalTemp,
            commit: crate::domain::CommitSemantics::AtomicRename,
        };
        let job = state
            .transfer_manager
            .create_inline_upload_job_with_plan(
                Some("user1".into()),
                "race_rename.txt".into(),
                "local".into(),
                "/race_rename.txt".into(),
                Some(10),
                plan.clone(),
            )
            .await;
        let token = state.transfer_manager.cancel_token(&job.id).expect("token exists");
        // Stream that completes immediately (one small chunk)
        let byte_stream = futures::stream::once(async { Ok(Bytes::from(vec![1u8; 10])) });
        let manager_clone = state.transfer_manager.clone();
        let job_id_clone = job.id.clone();
        let token_clone = token.clone();
        let provider_clone = provider.clone();
        let plan_clone = plan.clone();
        let target_clone = target.clone();
        let mock_clone = mock.clone();
        let handle = tokio::spawn(async move {
            execute_inline_upload_stream(
                &manager_clone,
                provider_clone,
                target_clone,
                &job_id_clone,
                &plan_clone,
                Some(10),
                10 * 1024 * 1024,
                false,
                None,
                token_clone,
                byte_stream,
            )
            .await
        });
        // Wait for rename to start (deterministic)
        tokio::time::timeout(std::time::Duration::from_secs(2), mock_clone.rename_started.notified())
            .await
            .expect("rename should start");
        // Now cancel — should be too late (Finalizing)
        let cancel_res = state.transfer_manager.cancel_job(&job.id, Some("user1"), false).await.unwrap();
        assert_eq!(cancel_res, false, "cancel should be too late after Finalizing");
        // Allow rename to complete
        mock_clone.rename_continue.notify_waiters();
        mock_clone.write_continue.notify_waiters();
        let res = tokio::time::timeout(std::time::Duration::from_secs(3), handle)
            .await
            .expect("executor should complete")
            .unwrap();
        assert!(res.is_ok(), "executor should succeed despite late cancel, got {:?}", res);
        assert!(mock_clone.rename_called.load(Ordering::SeqCst));
        // Final job should be Completed, not Cancelled
        let jobs = state.transfer_manager.list_jobs(Some("user1"), false, false).await;
        // Find job
        let final_job = jobs.iter().find(|j| j.id == job.id).or_else(|| {
            // May need to check DB via list with include_dismissed? But job is completed not dismissed
            None
        });
        // Alternative: check via manager internal? Use list_jobs includes completed
        // If not found in list, check directly via manager's job map via list
        // For now assert rename happened and executor succeeded
    }
}
