use crate::domain::FileMetadata;
use async_trait::async_trait;

#[async_trait]
pub trait FileMetadataCache: Send + Sync {
    async fn get(&self, connection_id: &str, path: &str) -> Option<FileMetadata>;
    async fn put(&self, connection_id: &str, path: &str, metadata: FileMetadata);
}
