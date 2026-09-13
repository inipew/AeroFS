use crate::domain::Connection;
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
