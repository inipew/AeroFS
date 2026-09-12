use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub type WsEvent = crate::events::DomainEvent;
pub type ReplayResult = crate::events::ReplayOutcome;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TransferType {
    Copy,
    Move,
    Upload,
    Sync,
}

impl TransferType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TransferType::Copy => "copy",
            TransferType::Move => "move",
            TransferType::Upload => "upload",
            TransferType::Sync => "sync",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "move" => TransferType::Move,
            "upload" => TransferType::Upload,
            "sync" => TransferType::Sync,
            _ => TransferType::Copy,
        }
    }
}

impl std::str::FromStr for TransferType {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(TransferType::from_str(s))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TransferStatus {
    Queued,
    Running,
    CancellationRequested,
    Cancelled,
    Interrupted,
    Completed,
    Failed,
}

impl TransferStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TransferStatus::Queued => "queued",
            TransferStatus::Running => "running",
            TransferStatus::CancellationRequested => "cancellation_requested",
            TransferStatus::Cancelled => "cancelled",
            TransferStatus::Interrupted => "interrupted",
            TransferStatus::Completed => "completed",
            TransferStatus::Failed => "failed",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "running" => TransferStatus::Running,
            "cancellation_requested" => TransferStatus::CancellationRequested,
            "cancelled" => TransferStatus::Cancelled,
            "interrupted" => TransferStatus::Interrupted,
            "completed" => TransferStatus::Completed,
            "failed" => TransferStatus::Failed,
            _ => TransferStatus::Queued,
        }
    }
}

impl std::str::FromStr for TransferStatus {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(TransferStatus::from_str(s))
    }
}

impl TransferStatus {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Cancelled | Self::Interrupted
        )
    }
    pub fn is_active(self) -> bool {
        matches!(
            self,
            Self::Queued | Self::Running | Self::CancellationRequested
        )
    }
    pub fn can_transition_to(self, next: Self) -> bool {
        match (self, next) {
            (Self::Queued, Self::Running)
            | (Self::Queued, Self::Cancelled)
            | (Self::Running, Self::CancellationRequested)
            | (Self::Running, Self::Completed)
            | (Self::Running, Self::Failed)
            | (Self::Running, Self::Interrupted)
            | (Self::CancellationRequested, Self::Cancelled)
            | (Self::Interrupted, Self::Queued)
            | (Self::Failed, Self::Queued) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TransferPhase {
    Preparing,
    Transferring,
    Finalizing,
    Verifying,
    CleaningUp,
    Completed,
}

impl TransferPhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            TransferPhase::Preparing => "preparing",
            TransferPhase::Transferring => "transferring",
            TransferPhase::Finalizing => "finalizing",
            TransferPhase::Verifying => "verifying",
            TransferPhase::CleaningUp => "cleaning_up",
            TransferPhase::Completed => "completed",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "transferring" => TransferPhase::Transferring,
            "finalizing" => TransferPhase::Finalizing,
            "verifying" => TransferPhase::Verifying,
            "cleaning_up" => TransferPhase::CleaningUp,
            "completed" => TransferPhase::Completed,
            _ => TransferPhase::Preparing,
        }
    }
}

impl std::str::FromStr for TransferPhase {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(TransferPhase::from_str(s))
    }
}

/// Execution mode for a transfer — Inline vs Background vs Resumable (§Upload-as-Transfer)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum TransferExecutionMode {
    #[default]
    Inline,
    Background,
    Resumable,
}

impl TransferExecutionMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Inline => "inline",
            Self::Background => "background",
            Self::Resumable => "resumable",
        }
    }
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "background" => Self::Background,
            "resumable" => Self::Resumable,
            _ => Self::Inline,
        }
    }
}
impl std::str::FromStr for TransferExecutionMode {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::from_str(s))
    }
}

/// Staging strategy — implementation detail of TransferEngine, not a separate subsystem
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum TransferStaging {
    #[default]
    None,
    LocalTemp,
    ProviderTemp,
}

impl TransferStaging {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::LocalTemp => "local_temp",
            Self::ProviderTemp => "provider_temp",
        }
    }
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "local_temp" => Self::LocalTemp,
            "provider_temp" => Self::ProviderTemp,
            _ => Self::None,
        }
    }
}
impl std::str::FromStr for TransferStaging {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::from_str(s))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TransferJob {
    pub id: String,
    pub user_id: Option<String>,
    pub name: String,
    pub transfer_type: TransferType,
    pub source_connection_id: String,
    pub source_path: String,
    pub destination_connection_id: String,
    pub destination_path: String,
    pub status: TransferStatus,
    pub phase: TransferPhase,
    /// Execution mode — Inline (sync HTTP), Background (queued), Resumable (checkpointed)
    #[serde(default)]
    pub execution_mode: TransferExecutionMode,
    /// Staging strategy — implementation detail of TransferEngine
    #[serde(default)]
    pub staging: TransferStaging,
    pub transferred_bytes: u64,
    pub total_bytes: u64,
    pub speed_bytes_per_sec: u64,
    pub eta_seconds: Option<u64>,
    pub checksum: Option<String>,
    pub error_message: Option<String>,
    pub dismissed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TransferCapabilities {
    pub can_cancel: bool,
    pub can_pause: bool,
    pub can_resume: bool,
    pub can_retry: bool,
}

pub fn is_cancellable_state(status: TransferStatus, phase: TransferPhase) -> bool {
    if status.is_terminal() || status == TransferStatus::CancellationRequested {
        return false;
    }
    !matches!(
        phase,
        TransferPhase::Finalizing
            | TransferPhase::Verifying
            | TransferPhase::CleaningUp
            | TransferPhase::Completed
    )
}

impl TransferJob {
    pub fn is_structurally_retryable(&self) -> bool {
        self.dismissed_at.is_none()
            && matches!(
                self.status,
                TransferStatus::Failed | TransferStatus::Interrupted
            )
            && !matches!(
                (self.transfer_type, self.execution_mode),
                (TransferType::Upload, TransferExecutionMode::Inline)
            )
    }

    pub fn can_cancel(&self) -> bool {
        is_cancellable_state(self.status, self.phase)
    }

    pub fn capabilities(&self) -> TransferCapabilities {
        TransferCapabilities {
            can_cancel: self.can_cancel(),
            can_pause: false,
            can_resume: false,
            can_retry: self.is_structurally_retryable(),
        }
    }

    pub fn to_response(&self) -> TransferJobResponse {
        TransferJobResponse {
            capabilities: self.capabilities(),
            job: self.clone(),
        }
    }
}

/// DTO for REST responses and WebSocket events, exposing TransferJob with calculated capabilities.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TransferJobResponse {
    #[serde(flatten)]
    pub job: TransferJob,
    pub capabilities: TransferCapabilities,
}

impl std::ops::Deref for TransferJobResponse {
    type Target = TransferJob;

    fn deref(&self) -> &Self::Target {
        &self.job
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CancelTransferError {
    #[error("Transfer job '{0}' not found")]
    NotFound(String),
    #[error("Permission denied: cannot cancel another user's transfer")]
    Unauthorized,
    #[error("Transfer job '{0}' cannot be cancelled in its current state")]
    NotCancellable(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

#[derive(Debug, thiserror::Error)]
pub enum RetryTransferError {
    #[error("Transfer job '{0}' not found")]
    NotFound(String),
    #[error("Permission denied: cannot retry another user's transfer")]
    Unauthorized,
    #[error("Cannot retry transfer '{0}': {1}")]
    InvalidStatus(String, String),
    #[error("Cannot retry transfer '{0}': source is not available on server")]
    SourceUnavailable(String),
    #[error("Storage provider '{0}' is not available")]
    ProviderUnavailable(String),
    #[error("Cannot retry transfer '{0}': transfer has been dismissed")]
    Dismissed(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Capabilities;
    use crate::transfer::planner::{TransferPlanner, UploadConstraints};

    #[test]
    fn test_upload_constraints_resumable_matrix() {
        let caps = Capabilities::local_default();
        let threshold = 10 * 1024 * 1024;
        // supports_resume=false → always Inline even if size > threshold
        let plan = TransferPlanner::plan_upload(
            &caps,
            UploadConstraints {
                total_hint: Some(100 * 1024 * 1024),
                supports_resume: false,
            },
            threshold,
            false,
        );
        assert_eq!(plan.execution_mode, TransferExecutionMode::Inline);
        // supports_resume=true + large → Resumable
        let plan = TransferPlanner::plan_upload(
            &caps,
            UploadConstraints {
                total_hint: Some(100 * 1024 * 1024),
                supports_resume: true,
            },
            threshold,
            false,
        );
        assert_eq!(plan.execution_mode, TransferExecutionMode::Resumable);
        // supports_resume=true + small → Inline
        let plan = TransferPlanner::plan_upload(
            &caps,
            UploadConstraints {
                total_hint: Some(1024 * 1024),
                supports_resume: true,
            },
            threshold,
            false,
        );
        assert_eq!(plan.execution_mode, TransferExecutionMode::Inline);
        // supports_resume=true + None → Inline
        let plan = TransferPlanner::plan_upload(
            &caps,
            UploadConstraints {
                total_hint: None,
                supports_resume: true,
            },
            threshold,
            false,
        );
        assert_eq!(plan.execution_mode, TransferExecutionMode::Inline);
    }

    #[test]
    fn test_staging_commit_matrix() {
        let caps_local = Capabilities::local_default();
        // local has atomic_rename true → LocalTemp staging, uses_staging true
        let plan = TransferPlanner::plan_upload(
            &caps_local,
            UploadConstraints::inline(Some(1024)),
            10 * 1024 * 1024,
            false,
        );
        assert_eq!(plan.staging, TransferStaging::LocalTemp);
        assert!(plan.uses_staging());
        // s3-like caps: atomic_rename false, atomic_write true → ProviderTemp but commit AtomicObjectPut
        // uses_staging true per spec (staging != None)
        let caps_s3 = Capabilities {
            atomic_write: true,
            atomic_rename: false,
            ..Default::default()
        };
        let plan = TransferPlanner::plan_upload(
            &caps_s3,
            UploadConstraints::inline(Some(1024)),
            10 * 1024 * 1024,
            false,
        );
        assert_eq!(plan.staging, TransferStaging::ProviderTemp);
        // per new spec uses_staging = staging != None, so true
        assert!(plan.uses_staging());
        // no atomic capabilities → None
        let caps_none = Capabilities::default();
        let plan = TransferPlanner::plan_upload(
            &caps_none,
            UploadConstraints::inline(Some(1024)),
            10 * 1024 * 1024,
            false,
        );
        assert_eq!(plan.staging, TransferStaging::None);
        assert!(!plan.uses_staging());
        assert!(plan
            .staging_path(
                &crate::domain::VfsPath::new("c", "/a/b.txt").unwrap(),
                "jid"
            )
            .is_none());
        // staging_path Some when uses_staging
        let plan_local = TransferPlanner::plan_upload(
            &caps_local,
            UploadConstraints::inline(Some(1024)),
            10 * 1024 * 1024,
            false,
        );
        let target = crate::domain::VfsPath::new("c", "/a/b.txt").unwrap();
        let sp = plan_local.staging_path(&target, "jid123").unwrap();
        assert!(sp.path.contains(".aerofs-part-jid123"));
    }

    #[test]
    fn test_capabilities_matrix_and_serialization() {
        let now = chrono::Utc::now();
        let base_job = TransferJob {
            id: "job_test_1".to_string(),
            user_id: Some("user1".to_string()),
            name: "test.txt".to_string(),
            transfer_type: TransferType::Copy,
            source_connection_id: "local".to_string(),
            source_path: "/src/test.txt".to_string(),
            destination_connection_id: "local".to_string(),
            destination_path: "/dst/test.txt".to_string(),
            status: TransferStatus::Queued,
            phase: TransferPhase::Preparing,
            execution_mode: TransferExecutionMode::Background,
            staging: TransferStaging::None,
            transferred_bytes: 0,
            total_bytes: 100,
            speed_bytes_per_sec: 0,
            eta_seconds: None,
            checksum: None,
            error_message: None,
            dismissed_at: None,
            created_at: now,
            updated_at: now,
        };

        // 1. Queued background copy: can_cancel = true, can_retry = false
        let caps = base_job.capabilities();
        assert!(caps.can_cancel);
        assert!(!caps.can_retry);
        assert!(!caps.can_pause);
        assert!(!caps.can_resume);

        // 2. Finalizing phase: can_cancel = false
        let mut fin_job = base_job.clone();
        fin_job.status = TransferStatus::Running;
        fin_job.phase = TransferPhase::Finalizing;
        assert!(!fin_job.capabilities().can_cancel);

        // 3. Failed background copy: can_cancel = false, can_retry = true
        let mut failed_copy = base_job.clone();
        failed_copy.status = TransferStatus::Failed;
        failed_copy.phase = TransferPhase::Completed;
        assert!(!failed_copy.capabilities().can_cancel);
        assert!(failed_copy.capabilities().can_retry);

        // 4. Failed inline upload: can_cancel = false, can_retry = false (Upload + Inline invariant)
        let mut failed_upload = base_job.clone();
        failed_upload.transfer_type = TransferType::Upload;
        failed_upload.execution_mode = TransferExecutionMode::Inline;
        failed_upload.status = TransferStatus::Failed;
        assert!(!failed_upload.capabilities().can_cancel);
        assert!(!failed_upload.capabilities().can_retry);

        // 5. Cancelled job: can_retry = false
        let mut cancelled_job = base_job.clone();
        cancelled_job.status = TransferStatus::Cancelled;
        assert!(!cancelled_job.capabilities().can_cancel);
        assert!(!cancelled_job.capabilities().can_retry);

        // 6. JSON serialization of TransferJobResponse flattens properly
        let response = base_job.to_response();
        let json = serde_json::to_value(&response).expect("serialize");
        assert_eq!(json["id"], "job_test_1");
        assert_eq!(json["status"], "queued");
        assert_eq!(json["capabilities"]["can_cancel"], true);
        assert_eq!(json["capabilities"]["can_retry"], false);
    }
}
