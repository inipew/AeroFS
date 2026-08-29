use crate::domain::FileMetadata;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Clone)]
struct CachedMetadata {
    metadata: FileMetadata,
    expires_at: Instant,
}

/// Thread-safe, bounded, short-TTL metadata cache to reduce remote roundtrips (S3/SFTP)
#[derive(Clone)]
pub struct MetadataCache {
    entries: Arc<RwLock<HashMap<String, CachedMetadata>>>,
    ttl: Duration,
}

impl Default for MetadataCache {
    fn default() -> Self {
        Self::new(Duration::from_secs(3))
    }
}

impl MetadataCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            ttl,
        }
    }

    fn make_key(connection_id: &str, path: &str) -> String {
        format!("{}:{}", connection_id, path.trim_end_matches('/'))
    }

    pub async fn get(&self, connection_id: &str, path: &str) -> Option<FileMetadata> {
        let key = Self::make_key(connection_id, path);
        let entries = self.entries.read().await;
        if let Some(cached) = entries.get(&key) {
            if Instant::now() < cached.expires_at {
                return Some(cached.metadata.clone());
            }
        }
        None
    }

    pub async fn put(&self, connection_id: &str, path: &str, metadata: FileMetadata) {
        let key = Self::make_key(connection_id, path);
        let mut entries = self.entries.write().await;
        // Bounded capacity cleanup
        if entries.len() > 10_000 {
            entries.retain(|_, v| Instant::now() < v.expires_at);
        }
        entries.insert(
            key,
            CachedMetadata {
                metadata,
                expires_at: Instant::now() + self.ttl,
            },
        );
    }

    pub async fn invalidate(&self, connection_id: &str, path: &str) {
        let key = Self::make_key(connection_id, path);
        let mut entries = self.entries.write().await;
        entries.remove(&key);
    }

    pub async fn invalidate_prefix(&self, connection_id: &str, path_prefix: &str) {
        let prefix = format!("{}:{}", connection_id, path_prefix.trim_end_matches('/'));
        let mut entries = self.entries.write().await;
        entries.retain(|k, _| !k.starts_with(&prefix));
    }

    pub async fn clear(&self) {
        let mut entries = self.entries.write().await;
        entries.clear();
    }
}
