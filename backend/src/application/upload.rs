//! UploadApplicationService — HTTP → TransferCommand translation (§Upload-as-Transfer)
//! Keeps VFS as owner of storage, TransferEngine as owner of lifecycle.

use crate::domain::VfsPath;
use crate::errors::{AppError, VfsError};
use crate::state::AppState;
use crate::transfer::{TransferExecutionMode, TransferStaging};

pub struct UploadApplicationService;

impl UploadApplicationService {
    /// Determine staging based on provider capabilities (implementation detail of TransferEngine)
    /// @deprecated — use `plan_for_upload` via `select_plan` instead.
    pub fn select_staging(caps: &crate::domain::Capabilities) -> TransferStaging {
        crate::transfer::planner::TransferPlanner::upload_staging(caps)
    }

    /// Determine execution mode based on size & inline threshold (small inline, large resumable)
    /// @deprecated — use `select_plan` instead.
    pub fn select_execution_mode(total: Option<u64>, inline_threshold: u64) -> TransferExecutionMode {
        crate::transfer::planner::TransferPlanner::upload_execution_mode(total, inline_threshold)
    }

    /// Unified planner — single source of truth for upload decisions (§P0-2).
    /// Returns TransferPlan that engine must execute without re-deciding.
    pub fn select_plan(
        caps: &crate::domain::Capabilities,
        total_hint: Option<u64>,
        inline_threshold: u64,
        target_exists: bool,
    ) -> crate::transfer::plan::TransferPlan {
        crate::transfer::planner::TransferPlanner::plan_upload(
            caps,
            total_hint,
            inline_threshold,
            target_exists,
        )
    }

    /// Validate target path & permissions, returning typed VfsPath
    pub fn validate_target(connection_id: &str, dest_path: &str) -> Result<VfsPath, AppError> {
        Ok(VfsPath::new(connection_id, dest_path)?)
    }

    /// Build TransferJob for an incoming upload (inline execution) using unified plan.
    /// Caller is responsible for streaming bytes and calling complete/fail,
    /// or prefer `execute_inline_stream` which owns the full lifecycle.
    pub async fn begin_inline_upload(
        state: &AppState,
        user_id: Option<String>,
        connection_id: &str,
        dest_path: &str,
        file_name: &str,
        total_hint: Option<u64>,
    ) -> Result<crate::transfer::TransferJob, AppError> {
        let provider = state.get_provider(connection_id).await.ok_or_else(|| {
            VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
        })?;
        let target_exists = {
            let vfs = VfsPath::new(connection_id, dest_path)?;
            provider.stat(&vfs).await.is_ok()
        };
        let inline_threshold = state.config.limits.max_editable_size;
        let plan = Self::select_plan(
            &provider.capabilities(),
            total_hint,
            inline_threshold,
            target_exists,
        );
        let job = state
            .transfer_manager
            .create_inline_upload_job_with_plan(
                user_id,
                file_name.to_string(),
                connection_id.to_string(),
                dest_path.to_string(),
                total_hint,
                plan,
            )
            .await;
        Ok(job)
    }

    /// Thin application-layer helper to execute an inline upload stream fully
    /// owned by TransferEngine lifecycle (handler becomes thin HTTP adapter).
    /// Handles: staging selection via plan, duplex pipe, write_stream, progress,
    /// atomic promotion, permission inheritance, cache invalidation, audit, completion.
    ///
    /// `field` is a mutable multipart field from axum — read chunk-by-chunk.
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
        use tokio::io::AsyncWriteExt;

        // 1. Build plan (single source of truth) — needs target_exists for WriteStrategy
        let target_exists = provider.stat(&target).await.is_ok();
        let inline_threshold = state.config.limits.max_editable_size;
        let plan = Self::select_plan(
            &provider.capabilities(),
            total_hint,
            inline_threshold,
            target_exists,
        );

        // 2. Create TransferJob owned by TransferEngine
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

        // 3. Resolve staging target via plan (canonical `.<file>.aerofs-part-<job_id>`)
        let write_target = plan
            .staging_path(&target, &job_id)
            .unwrap_or_else(|| target.clone());
        let use_staging = plan.use_staging_file;

        // 4. Resolve permission inheritance before write
        let target_perms = crate::domain::resolve_destination_permissions(
            provider,
            &target,
            false,
            crate::domain::PermissionInheritanceMode::InheritExistingOrParent,
        )
        .await;

        // 5. Disk-space guard for local
        if connection_id == "local" {
            if let Some(free_bytes) =
                get_available_disk_space(&state.config.filesystem.default_local_root)
            {
                if free_bytes < 10 * 1024 * 1024 {
                    let msg = format!(
                        "Local filesystem storage full: only {} MB free",
                        free_bytes / (1024 * 1024)
                    );
                    state
                        .transfer_manager
                        .fail_inline_job(&job_id, msg.clone())
                        .await;
                    return Err(AppError::InsufficientStorage(msg));
                }
            }
        }

        // 6. Acquire upload lock (prevent concurrent uploads to same path)
        // Note: guard must live for duration of upload
        let _guard = state.upload_locks.try_acquire(connection_id, &target.path).await.map_err(|e| {
            let msg = e.to_string();
            // fail job synchronously — we spawned it already
            msg
        });
        if let Err(msg) = &_guard {
            state.transfer_manager.fail_inline_job(&job_id, msg.clone()).await;
            return Err(AppError::Conflict(msg.clone()));
        }
        let _guard = _guard.unwrap();

        // 7. Duplex pipe → provider.write_stream (engine owns VFS interaction)
        let cancel_token = tokio_util::sync::CancellationToken::new();
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
        let start_time = std::time::Instant::now();
        let mut last_emit = std::time::Instant::now();
        // Fix P0-3: use actual total_hint, not uploaded_bytes==uploaded_bytes.
        // If total unknown → 0 (indeterminate) so UI doesn't show 100% prematurely.
        let total_for_progress = total_hint.unwrap_or(0);

        while let Some(chunk) = match field.chunk().await {
            Ok(c) => c,
            Err(e) => {
                stream_err = Some(AppError::BadRequest(format!("Upload stream error: {}", e)));
                None
            }
        } {
            uploaded_bytes += chunk.len() as u64;
            if uploaded_bytes > max_upload_bytes {
                stream_err = Some(AppError::PayloadTooLarge(format!(
                    "Uploaded file exceeded maximum upload size limit of {} bytes",
                    max_upload_bytes
                )));
                break;
            }
            if let Err(e) = duplex_writer.write_all(&chunk).await {
                stream_err = Some(AppError::Internal(anyhow::anyhow!(
                    "Failed writing upload chunk: {}",
                    e
                )));
                break;
            }
            if last_emit.elapsed().as_millis() >= 100 {
                let elapsed = start_time.elapsed().as_secs_f64();
                let speed = if elapsed > 0.05 {
                    (uploaded_bytes as f64 / elapsed) as u64
                } else {
                    0
                };
                state
                    .transfer_manager
                    .update_inline_progress(&job_id, uploaded_bytes, total_for_progress, speed, None)
                    .await;
                last_emit = std::time::Instant::now();
            }
        }
        drop(duplex_writer);

        if let Some(err) = stream_err {
            cancel_token.cancel();
            let _ = write_handle.await;
            if use_staging || !target_exists {
                let _ = provider.delete(&write_target).await;
            }
            let msg = err.to_string();
            state.transfer_manager.fail_inline_job(&job_id, msg).await;
            return Err(err);
        }

        let write_res = write_handle
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Upload worker task error: {}", e)))?;

        if let Err(e) = write_res {
            if use_staging || !target_exists {
                let _ = provider.delete(&write_target).await;
            }
            let msg = e.to_string();
            state.transfer_manager.fail_inline_job(&job_id, msg.clone()).await;
            return Err(AppError::from(e));
        }

        if use_staging {
            if let Err(rename_err) = provider.rename(&write_target, &target).await {
                let _ = provider.delete(&write_target).await;
                let msg = format!(
                    "Failed to promote staging file to final destination '{}': {}",
                    target.path, rename_err
                );
                state.transfer_manager.fail_inline_job(&job_id, msg.clone()).await;
                return Err(AppError::Internal(anyhow::anyhow!(msg)));
            }
        }

        if let Some(ref perms) = target_perms {
            let _ = provider.set_permissions(&target, perms).await;
        }

        state.metadata_cache.invalidate(&target.connection_id, &target.path).await;

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
