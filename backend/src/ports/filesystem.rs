use crate::domain::ConnectionId;
use crate::errors::{AppError, VfsError};
use crate::vfs::FileSystem;
use async_trait::async_trait;
use std::sync::Arc;

#[async_trait]
pub trait FileSystemResolver: Send + Sync {
    async fn resolve(&self, connection: &ConnectionId) -> Result<Arc<dyn FileSystem>, VfsError>;
}

#[derive(Debug, Clone)]
pub struct ConnectionStorageDescriptor {
    pub name: String,
    pub provider: String,
    pub host: Option<String>,
    pub port: Option<i64>,
}

#[async_trait]
pub trait ConnectionStorageMetadata: Send + Sync {
    async fn get(
        &self,
        connection_id: &str,
    ) -> Result<Option<ConnectionStorageDescriptor>, AppError>;
}
