//! UploadApplicationService — HTTP → TransferCommand translation (§Upload-as-Transfer)
//! Thin orchestrator: validate → lock → create TransferJob → delegate streaming to TransferExecutor.
//! Executor (transfer::executor) owns duplex/write_stream/progress/staging/commit/cleanup.

use crate::domain::VfsPath;
use crate::errors::AppError;
use crate::state::AppState;

pub struct UploadApplicationService;

impl UploadApplicationService {
    /// Unified planner — single source of truth for upload decisions (§P0-2).
    /// Uses UploadConstraints{total_hint, supports_resume:false} for current multipart inline.
    pub fn select_plan(
        caps: &crate::domain::Capabilities,
        total_hint: Option<u64>,
        inline_threshold: u64,
        target_exists: bool,
    ) -> crate::transfer::plan::TransferPlan {
        crate::transfer::planner::TransferPlanner::plan_upload(
            caps,
            crate::transfer::planner::UploadConstraints::inline(total_hint),
            inline_threshold,
            target_exists,
        )
    }

    /// Validate target path & permissions, returning typed VfsPath
    pub fn validate_target(connection_id: &str, dest_path: &str) -> Result<VfsPath, AppError> {
        Ok(VfsPath::new(connection_id, dest_path)?)
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
        let plan = Self::select_plan(
            &provider.capabilities(),
            total_hint,
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

        // 3b. Fetch manager-owned cancellation token (P0) — clone, never create new
        let cancel_token = state
            .transfer_manager
            .cancel_token(&job_id)
            .unwrap_or_else(tokio_util::sync::CancellationToken::new);
        // Race guard: cancel fired between create and executor start → abort with no commit
        if cancel_token.is_cancelled() {
            // No staging file yet, but ensure job lifecycle respects cancellation
            // Let cancel_job's status win; convert to cancelled error without overwriting with Failed
            return Err(AppError::Internal(anyhow::anyhow!("Upload cancelled before start")));
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
            target.clone(),
            &job_id,
            &plan,
            total_hint,
            max_upload_bytes,
            target_exists,
            target_perms,
            cancel_token.clone(),
            byte_stream,
        )
        .await;

        // 6. Cancellation-aware completion: if token is cancelled, do NOT overwrite Cancelled with Failed
        let is_cancelled = cancel_token.is_cancelled()
            || exec_result
                .as_ref()
                .err()
                .map(|e| e.to_string().contains("cancelled"))
                .unwrap_or(false);
        match exec_result {
            Ok(_) => {
                // Check cancelled again before commit (handles cancel during last bytes)
                if is_cancelled {
                    // Executor already cleaned staging; ensure no commit happened
                    return Err(AppError::Internal(anyhow::anyhow!("Upload cancelled")));
                }
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
                    Some(&format!("Uploaded: {} via Transfer {}", target.path, job_id)),
                )
                .await;

                state.transfer_manager.complete_inline_job(&job_id, None).await;
                Ok(target.path)
            }
            Err(e) => {
                if is_cancelled {
                    // Let manager's cancel_job status (Cancelled/CancellationRequested) win; ensure token is cancelled
                    // Avoid calling fail_inline_job which would set Failed and overwrite Cancelled
                    return Err(AppError::Internal(anyhow::anyhow!("Upload cancelled")));
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
