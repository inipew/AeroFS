use crate::domain::ConnectionId;
use crate::errors::VfsError;
use crate::vfs::FileSystem;
use async_trait::async_trait;
use std::sync::Arc;

#[async_trait]
pub trait FileSystemResolver: Send + Sync {
    async fn resolve(&self, connection: &ConnectionId) -> Result<Arc<dyn FileSystem>, VfsError>;
}
