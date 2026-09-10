//! UploadApplicationService — HTTP → TransferCommand translation (§Upload-as-Transfer)
//! Thin orchestrator: validate → lock → create TransferJob → delegate streaming to TransferExecutor.
//! Executor (transfer::executor) owns duplex/write_stream/progress/staging/commit/cleanup.

use crate::domain::VfsPath;
use crate::errors::AppError;
use crate::state::AppState;
use bytes::Bytes;
use futures::Stream;
use std::sync::Arc;

pub struct UploadApplicationService;

impl UploadApplicationService {
    /// Validate target path & permissions, returning typed VfsPath
    pub fn validate_target(connection_id: &str, dest_path: &str) -> Result<VfsPath, AppError> {
        Ok(VfsPath::new(connection_id, dest_path)?)
    }

    /// Admit an upload before its body is sent. The returned session keeps the
    /// destination lock until the matching stream completes or is cancelled.
    pub async fn create_session(
        state: &AppState,
        user_id: &str,
        connection_id: &str,
        provider: &Arc<dyn crate::vfs::FileSystem>,
        target: VfsPath,
        file_name: String,
        total_bytes: Option<u64>,
    ) -> Result<crate::services::UploadSession, AppError> {
        let target_exists = provider.stat(&target).await.is_ok();
        let plan = crate::transfer::planner::TransferPlanner::plan_upload(
            &provider.capabilities(),
            crate::transfer::planner::UploadConstraints::inline(total_bytes),
            state.config.limits.max_editable_size,
            target_exists,
        );
        let target_perms = crate::domain::resolve_destination_permissions(
            provider,
            &target,
            false,
            crate::domain::PermissionInheritanceMode::InheritExistingOrParent,
        )
        .await;
        if connection_id == "local" {
            if let Some(free_bytes) =
                get_available_disk_space(&state.config.filesystem.default_local_root)
            {
                if free_bytes < 10 * 1024 * 1024 {
                    return Err(AppError::InsufficientStorage(format!(
                        "Local filesystem storage full: only {} MB free",
                        free_bytes / (1024 * 1024)
                    )));
                }
            }
        }

        // Acquire before creation: a job represents an accepted transfer only.
        let guard = state
            .upload_locks
            .try_acquire(connection_id, &target.path)
            .await?;
        let job = state
            .transfer_manager
            .create_inline_upload_job_with_plan(
                Some(user_id.to_string()),
                file_name.clone(),
                connection_id.to_string(),
                target.path.clone(),
                total_bytes,
                plan.clone(),
            )
            .await;
        let session = crate::services::UploadSession {
            job_id: job.id.clone(),
            user_id: user_id.to_string(),
            connection_id: connection_id.to_string(),
            target,
            file_name,
            total_bytes,
            max_upload_bytes: state.config.limits.max_upload_size,
            target_exists,
            target_perms,
            plan,
        };
        // Avoid a second acquire; transfer the guard into the reservation map.
        state
            .upload_locks
            .reserve_existing(session.clone(), guard)
            .await;
        Ok(session)
    }

    pub async fn execute_session_stream<S>(
        state: &AppState,
        user_id: &str,
        connection_id: &str,
        job_id: &str,
        byte_stream: S,
    ) -> Result<String, AppError>
    where
        S: Stream<Item = Result<Bytes, AppError>> + Send,
    {
        let session = state.upload_locks.claim(job_id, user_id).await?;
        if session.connection_id != connection_id {
            state.upload_locks.release(job_id).await;
            state.transfer_manager.cancel_inline_job(job_id).await;
            return Err(AppError::NotFound(format!(
                "Upload session '{}' not found",
                job_id
            )));
        }
        let provider = state
            .get_provider(&session.connection_id)
            .await
            .ok_or_else(|| {
                AppError::Vfs(crate::errors::VfsError::ConnectionError(format!(
                    "Connection '{}' not found",
                    session.connection_id
                )))
            })?;
        let cancel_token = match state.transfer_manager.cancel_token(&session.job_id) {
            Some(token) => token,
            None => {
                state.upload_locks.release(job_id).await;
                state
                    .transfer_manager
                    .fail_inline_job(job_id, "transfer cancellation token missing".into())
                    .await;
                return Err(AppError::Internal(anyhow::anyhow!(
                    "transfer cancellation token missing"
                )));
            }
        };
        let result = crate::transfer::executor::execute_inline_upload_stream(
            &state.transfer_manager,
            provider,
            crate::transfer::executor::InlineUploadContext {
                target: session.target.clone(),
                job_id: session.job_id.clone(),
                plan: session.plan,
                total_hint: session.total_bytes,
                max_bytes: session.max_upload_bytes,
                target_exists: session.target_exists,
                target_perms: session.target_perms,
            },
            cancel_token,
            byte_stream,
        )
        .await;
        state.upload_locks.release(job_id).await;
        match result {
            Ok(_) => {
                state
                    .metadata_cache
                    .invalidate(&session.connection_id, &session.target.path)
                    .await;
                crate::auth::record_audit_log(
                    &state.db,
                    Some(user_id),
                    "FILE_UPLOAD",
                    Some(&session.connection_id),
                    Some(&session.target.path),
                    "SUCCESS",
                    None,
                    Some(&format!(
                        "Uploaded: {} via Transfer {}",
                        session.target.path, session.job_id
                    )),
                )
                .await;
                state
                    .transfer_manager
                    .complete_inline_job(job_id, None)
                    .await;
                Ok(session.target.path)
            }
            Err(error) => {
                if matches!(error, AppError::Cancelled(_)) {
                    state.transfer_manager.cancel_inline_job(job_id).await;
                } else {
                    state
                        .transfer_manager
                        .fail_inline_job(job_id, error.to_string())
                        .await;
                }
                Err(error)
            }
        }
    }

    /// Execute an inline upload stream with correct ordering:
    /// validate → lock → create TransferJob → TransferExecutor::execute → cache/audit → complete
    /// Pre-execution failures (disk full, lock conflict, invalid path) do NOT create a TransferJob.
    #[allow(clippy::too_many_arguments)]
    pub async fn execute_inline_stream(
        state: &AppState,
        user_id: &str,
        connection_id: &str,
        provider: &std::sync::Arc<dyn crate::vfs::FileSystem>,
        target: VfsPath,
        file_name: &str,
        total_hint: Option<u64>,
        max_upload_bytes: u64,
        field: &mut axum::extract::multipart::Field<'_>,
    ) -> Result<String, AppError> {
        // 1. Pre-checks before job creation (no TransferJob for pre-execution failures)
        let target_exists = provider.stat(&target).await.is_ok();
        let inline_threshold = state.config.limits.max_editable_size;
        let plan = crate::transfer::planner::TransferPlanner::plan_upload(
            &provider.capabilities(),
            crate::transfer::planner::UploadConstraints::inline(total_hint),
            inline_threshold,
            target_exists,
        );

        // Resolve permission inheritance before write (needed by executor)
        let target_perms = crate::domain::resolve_destination_permissions(
            provider,
            &target,
            false,
            crate::domain::PermissionInheritanceMode::InheritExistingOrParent,
        )
        .await;

        // Disk-space guard for local (pre-lock)
        if connection_id == "local" {
            if let Some(free_bytes) =
                get_available_disk_space(&state.config.filesystem.default_local_root)
            {
                if free_bytes < 10 * 1024 * 1024 {
                    return Err(AppError::InsufficientStorage(format!(
                        "Local filesystem storage full: only {} MB free",
                        free_bytes / (1024 * 1024)
                    )));
                }
            }
        }

        // 2. Acquire upload lock BEFORE creating job — fail fast without TransferJob
        let _guard = state
            .upload_locks
            .try_acquire(connection_id, &target.path)
            .await?;

        // 3. Create TransferJob only after lock succeeds (job means transfer accepted for execution)
        let job = state
            .transfer_manager
            .create_inline_upload_job_with_plan(
                Some(user_id.to_string()),
                file_name.to_string(),
                connection_id.to_string(),
                target.path.clone(),
                total_hint,
                plan.clone(),
            )
            .await;
        let job_id = job.id.clone();

        // 3b. Fetch manager-owned cancellation token (P0) — clone, never create fallback
        let cancel_token = match state.transfer_manager.cancel_token(&job_id) {
            Some(token) => token,
            None => {
                let error = "transfer cancellation token missing".to_string();
                state
                    .transfer_manager
                    .fail_inline_job(&job_id, error.clone())
                    .await;
                return Err(AppError::Internal(anyhow::anyhow!(error)));
            }
        };
        // Race guard: cancel fired between create and executor start → abort with no commit
        if cancel_token.is_cancelled() {
            state.transfer_manager.cancel_inline_job(&job_id).await;
            return Err(AppError::Cancelled("upload cancelled before start".into()));
        }

        // 4. Adapt axum Field → Stream<Item=Result<Bytes, AppError>> (application boundary adapter)
        //    Transfer layer must not depend on axum.
        let byte_stream = futures::stream::unfold(field, |f| async move {
            match f.chunk().await {
                Ok(Some(bytes)) => Some((Ok(bytes), f)),
                Ok(None) => None,
                Err(e) => Some((
                    Err(AppError::BadRequest(format!("Upload stream error: {}", e))),
                    f,
                )),
            }
        });

        // 5. Delegate pure VFS streaming to TransferExecutor (manager-owned token)
        let exec_result = crate::transfer::executor::execute_inline_upload_stream(
            &state.transfer_manager,
            provider.clone(),
            crate::transfer::executor::InlineUploadContext {
                target: target.clone(),
                job_id: job_id.clone(),
                plan,
                total_hint,
                max_bytes: max_upload_bytes,
                target_exists,
                target_perms,
            },
            cancel_token.clone(),
            byte_stream,
        )
        .await;

        // 6. Cancellation-aware completion: Opsi X — once Finalizing, commit wins even if token cancelled (too late)
        match exec_result {
            Ok(_) => {
                // Executor returned Ok → it successfully entered Finalizing and committed (rename succeeded)
                // Even if token got cancelled during rename, Opsi X says VFS commit wins → Completed
                // Post-success: cache invalidation (application concern, not transfer executor)
                state
                    .metadata_cache
                    .invalidate(&target.connection_id, &target.path)
                    .await;

                crate::auth::record_audit_log(
                    &state.db,
                    Some(user_id),
                    "FILE_UPLOAD",
                    Some(connection_id),
                    Some(&target.path),
                    "SUCCESS",
                    None,
                    Some(&format!(
                        "Uploaded: {} via Transfer {}",
                        target.path, job_id
                    )),
                )
                .await;

                state
                    .transfer_manager
                    .complete_inline_job(&job_id, None)
                    .await;
                Ok(target.path)
            }
            Err(e) => {
                if matches!(e, AppError::Cancelled(_)) {
                    state.transfer_manager.cancel_inline_job(&job_id).await;
                    return Err(e);
                }
                let msg = e.to_string();
                state.transfer_manager.fail_inline_job(&job_id, msg).await;
                Err(e)
            }
        }
    }
}

#[cfg(unix)]
fn get_available_disk_space(path: &std::path::Path) -> Option<u64> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let c_path = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) } == 0 {
        Some((stat.f_bavail as u64) * (stat.f_frsize as u64))
    } else {
        None
    }
}

#[cfg(not(unix))]
fn get_available_disk_space(_path: &std::path::Path) -> Option<u64> {
    None
}
