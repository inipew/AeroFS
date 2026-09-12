use crate::domain::{Actor, ConnectionId};
use crate::errors::AppError;
use async_trait::async_trait;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileAction {
    List,
    Read,
    Download,
    Create,
    Write,
    Upload,
    Delete,
}

#[async_trait]
pub trait Authorization: Send + Sync {
    async fn authorize(
        &self,
        actor: &Actor,
        connection: &ConnectionId,
        action: FileAction,
    ) -> Result<(), AppError>;
}
