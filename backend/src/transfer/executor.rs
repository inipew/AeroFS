//! TransferExecutor — pure VFS streaming owned by transfer layer.
//! No dependency on AppState / Axum / HTTP. Caller (application) adapts
//! `axum::extract::multipart::Field` → `Stream<Item=Result<Bytes, AppError>>`
//! and handles lock ordering + job creation before calling here.

use crate::domain::VfsPath;
use crate::errors::{AppError, VfsError};
use crate::transfer::plan::TransferPlan;
use crate::transfer::TransferManager;
use crate::vfs::FileSystem;
use axum::body::Bytes;
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

    // No commit if cancelled after stream but before rename (P0: must be impossible to commit after cancel)
    if cancel_token.is_cancelled() {
        if use_staging {
            let _ = provider.delete(&write_target).await;
        } else if !target_exists {
            let _ = provider.delete(&target).await;
        }
        return Err(AppError::Internal(anyhow::anyhow!("Upload cancelled")));
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
