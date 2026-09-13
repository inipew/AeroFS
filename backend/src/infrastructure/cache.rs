use crate::domain::FileMetadata;
use crate::ports::cache::FileMetadataCache;
use crate::services::MetadataCache;
use async_trait::async_trait;

#[async_trait]
impl FileMetadataCache for MetadataCache {
    async fn get(&self, connection_id: &str, path: &str) -> Option<FileMetadata> {
        MetadataCache::get(self, connection_id, path).await
    }

    async fn put(&self, connection_id: &str, path: &str, metadata: FileMetadata) {
        MetadataCache::put(self, connection_id, path, metadata).await;
    }
}
