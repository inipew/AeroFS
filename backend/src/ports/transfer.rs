use crate::domain::{Actor, ConnectionId};
use crate::errors::AppError;
use crate::transfer::TransferType;
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
    async fn submitted(
        &self,
        actor: &Actor,
        submission: &TransferSubmission,
        job_id: &str,
    );
}
