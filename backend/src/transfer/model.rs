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
                total_hint: Some(1 * 1024 * 1024),
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
        let mut caps_s3 = Capabilities::default();
        caps_s3.atomic_write = true;
        caps_s3.atomic_rename = false;
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
        assert!(plan.staging_path(&crate::domain::VfsPath::new("c", "/a/b.txt").unwrap(), "jid").is_none());
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
}
