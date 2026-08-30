//! UploadApplicationService — HTTP → TransferCommand translation (§Upload-as-Transfer)
//! Keeps VFS as owner of storage, TransferEngine as owner of lifecycle.

use crate::domain::VfsPath;
use crate::errors::{AppError, VfsError};
use crate::state::AppState;
use crate::transfer::{TransferExecutionMode, TransferStaging};

pub struct UploadApplicationService;

impl UploadApplicationService {
    /// Determine staging based on provider capabilities (implementation detail of TransferEngine)
    pub fn select_staging(caps: &crate::domain::Capabilities) -> TransferStaging {
        crate::transfer::planner::TransferPlanner::upload_staging(caps)
    }

    /// Determine execution mode based on size & inline threshold (small inline, large resumable)
    pub fn select_execution_mode(total: Option<u64>, inline_threshold: u64) -> TransferExecutionMode {
        crate::transfer::planner::TransferPlanner::upload_execution_mode(total, inline_threshold)
    }

    /// Validate target path & permissions, returning typed VfsPath
    pub fn validate_target(connection_id: &str, dest_path: &str) -> Result<VfsPath, AppError> {
        Ok(VfsPath::new(connection_id, dest_path)?)
    }

    /// Build TransferJob for an incoming upload (inline execution).
    /// Caller is responsible for streaming bytes and calling complete/fail.
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
        let staging = Self::select_staging(&provider.capabilities());
        let inline_threshold = state.config.limits.max_editable_size;
        let execution_mode = Self::select_execution_mode(total_hint, inline_threshold);
        // For current HTTP multipart we always run Inline (stream is ephemeral);
        // Resumable is reserved for future chunked/resumable endpoint.
        let execution_mode = match execution_mode {
            TransferExecutionMode::Resumable => TransferExecutionMode::Inline,
            m => m,
        };
        let job = state
            .transfer_manager
            .create_inline_upload_job(
                user_id,
                file_name.to_string(),
                connection_id.to_string(),
                dest_path.to_string(),
                total_hint,
                staging,
                execution_mode,
            )
            .await;
        Ok(job)
    }
}
