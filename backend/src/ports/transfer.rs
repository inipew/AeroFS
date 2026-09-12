use crate::domain::{Actor, ConnectionId};
use crate::errors::AppError;
use crate::transfer::{TransferJobResponse, TransferType};
use async_trait::async_trait;

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
