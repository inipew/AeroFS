use crate::domain::{Capabilities, Connection};
use crate::errors::AppError;
use async_trait::async_trait;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretMutation {
    Keep,
    Replace(String),
    Clear,
}

#[derive(Debug, thiserror::Error)]
pub enum ConnectionSecretError {
    #[error("credential persistence error: {0}")]
    Persistence(String),
    #[error("credential decryption error: {0}")]
    Decryption(String),
}

#[async_trait]
pub trait ConnectionRepository: Send + Sync {
    async fn local_root_override(&self) -> Result<Option<PathBuf>, AppError>;
    async fn load_enabled(&self) -> Result<Vec<Connection>, AppError>;
    async fn load_secret(&self, id: &str) -> Result<Option<String>, ConnectionSecretError>;

    async fn can_read(&self, user_id: &str, id: &str) -> Result<bool, AppError>;
    async fn list(&self, user_id: Option<&str>, is_admin: bool) -> Result<Vec<Connection>, AppError>;
    async fn get(&self, id: &str) -> Result<Option<Connection>, AppError>;
    async fn exists(&self, id: &str) -> Result<bool, AppError>;

    async fn create(&self, connection: &Connection, secret: Option<&str>) -> Result<(), AppError>;
    async fn update(
        &self,
        connection: &Connection,
        secret: SecretMutation,
    ) -> Result<(), AppError>;
    async fn disable(&self, id: &str) -> Result<(), AppError>;

    /// Atomically fence persisted active transfer rows and remove connection state.
    /// Returns false when the connection no longer exists.
    async fn delete_with_transfer_barrier(&self, id: &str) -> Result<bool, AppError>;
}

#[derive(Debug, Clone)]
pub struct ConnectionRuntimeInfo {
    pub active: bool,
    pub error_message: Option<String>,
    pub capabilities: Option<Capabilities>,
}

#[async_trait]
pub trait PreparedConnectionRuntime: Send {
    /// Publish the already-validated provider. Activation is intentionally
    /// infallible: all provider construction work must happen during prepare.
    async fn activate(self: Box<Self>);
}

#[async_trait]
pub trait DetachedConnectionRuntime: Send {
    /// Restore the exact previously unpublished runtime after a failed durable
    /// teardown. This keeps rollback details out of the application service.
    async fn restore(self: Box<Self>);
}

#[async_trait]
pub trait ConnectionRuntime: Send + Sync {
    async fn validate_target(&self, host: Option<&str>, port: Option<u16>) -> Result<(), AppError>;

    async fn prepare_local(
        &self,
        root: &std::path::Path,
    ) -> Result<Box<dyn PreparedConnectionRuntime>, AppError>;

    async fn prepare(
        &self,
        connection: &Connection,
        secret: Option<&str>,
    ) -> Result<Box<dyn PreparedConnectionRuntime>, AppError>;

    async fn set_error(&self, id: &str, error: &str);
    async fn info(&self, id: &str) -> ConnectionRuntimeInfo;
    async fn detach(&self, id: &str) -> Option<Box<dyn DetachedConnectionRuntime>>;
    async fn remove(&self, id: &str);
    async fn test(&self, id: &str) -> Result<u64, AppError>;
}

#[async_trait]
pub trait ConnectionEffects: Send + Sync {
    async fn cancel_active_transfers(&self, id: &str) -> Result<(), AppError>;
    async fn invalidate_metadata(&self, id: &str);
}
