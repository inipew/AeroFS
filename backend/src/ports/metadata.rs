use crate::domain::FileMetadata;
use async_trait::async_trait;

/// Narrow application-facing metadata cache contract.
///
/// The application/service layer depends on this port rather than the concrete
/// cache implementation so cache policy and storage remain infrastructure
/// concerns.
#[async_trait]
pub trait MetadataCachePort: Send + Sync {
    async fn get(&self, connection_id: &str, path: &str) -> Option<FileMetadata>;
    async fn put(&self, connection_id: &str, path: &str, metadata: FileMetadata);
}
