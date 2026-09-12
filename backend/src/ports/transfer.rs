use crate::domain::{Actor, ConnectionId};
use crate::errors::AppError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TransferType {
    Copy,
    Move,
    Upload,
    Sync,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum TransferExecutionMode {
    #[default]
    Inline,
    Background,
    Resumable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum TransferStaging {
    #[default]
    None,
    LocalTemp,
    ProviderTemp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TransferCapabilities {
    pub can_cancel: bool,
    pub can_pause: bool,
    pub can_resume: bool,
    pub can_retry: bool,
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
    #[serde(default)]
    pub execution_mode: TransferExecutionMode,
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

#[derive(Debug, Clone)]
pub struct TransferSubmission {
    pub user_id: Option<String>,
    pub name: String,
    pub transfer_type: TransferType,
    pub source_connection: ConnectionId,
    pub source_path: String,
    pub destination_connection: ConnectionId,
    pub destination_path: String,
}

#[async_trait]
pub trait TransferQueue: Send + Sync {
    async fn submit(&self, submission: TransferSubmission) -> Result<String, AppError>;
}

#[async_trait]
pub trait TransferEffects: Send + Sync {
    async fn submitted(&self, actor: &Actor, submission: &TransferSubmission, job_id: &str);
}

/// Query/control boundary used by transport adapters. Implementations own the
/// persistence, ownership, permission revalidation, audit and transfer-engine
/// details so HTTP handlers only map DTOs to application calls.
#[async_trait]
pub trait TransferControl: Send + Sync {
    async fn list(&self, actor: &Actor) -> Result<Vec<TransferJobResponse>, AppError>;
    async fn cancel(&self, actor: &Actor, job_id: &str) -> Result<(), AppError>;
    async fn retry(&self, actor: &Actor, job_id: &str) -> Result<(), AppError>;
    async fn dismiss(&self, actor: &Actor, job_id: &str) -> Result<(), AppError>;
    async fn clear_finished(&self, actor: &Actor) -> Result<usize, AppError>;
}
