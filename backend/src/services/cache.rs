use crate::domain::FileMetadata;
use crate::errors::AppError;
use crate::ports::cache::FileMetadataCache;
use async_trait::async_trait;
use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, RwLock};

const MAX_METADATA_ENTRIES: usize = 10_000;

#[derive(Clone)]
struct CachedMetadata {
    metadata: FileMetadata,
    expires_at: Instant,
}

type InFlightSender = broadcast::Sender<Result<FileMetadata, String>>;
type InFlightMap = Arc<StdMutex<HashMap<String, InFlightSender>>>;

struct InFlightLeader {
    key: String,
    map: InFlightMap,
    armed: bool,
}

impl InFlightLeader {
    fn complete(&mut self, result: &Result<FileMetadata, AppError>) {
        let sender = self
            .map
            .lock()
            .ok()
            .and_then(|mut in_flight| in_flight.remove(&self.key));
        if let Some(tx) = sender {
            let shared = match result {
                Ok(metadata) => Ok(metadata.clone()),
                Err(error) => Err(error.to_string()),
            };
            let _ = tx.send(shared);
        }
        self.armed = false;
    }
}

impl Drop for InFlightLeader {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        if let Ok(mut in_flight) = self.map.lock() {
            if let Some(tx) = in_flight.remove(&self.key) {
                let _ = tx.send(Err("metadata fetch cancelled before completion".into()));
            }
        }
    }
}

/// Thread-safe, hard-bounded, short-TTL metadata cache with single-flight
/// request coalescing for remote storage (S3/SFTP).
#[derive(Clone)]
pub struct MetadataCache {
    entries: Arc<RwLock<HashMap<String, CachedMetadata>>>,
    in_flight: InFlightMap,
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
            in_flight: Arc::new(StdMutex::new(HashMap::new())),
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
        let now = Instant::now();

        if entries.len() >= MAX_METADATA_ENTRIES && !entries.contains_key(&key) {
            entries.retain(|_, value| now < value.expires_at);
        }
        if entries.len() >= MAX_METADATA_ENTRIES && !entries.contains_key(&key) {
            if let Some(oldest_key) = entries
                .iter()
                .min_by_key(|(_, value)| value.expires_at)
                .map(|(key, _)| key.clone())
            {
                entries.remove(&oldest_key);
            }
        }

        entries.insert(
            key,
            CachedMetadata {
                metadata,
                expires_at: now + self.ttl,
            },
        );
    }

    /// Single-flight coalesced fetch. A leader token owns the in-flight slot;
    /// dropping/cancelling the leader future removes that slot synchronously so
    /// later callers cannot become stuck behind an abandoned request.
    pub async fn get_or_fetch<F, Fut>(
        &self,
        connection_id: &str,
        path: &str,
        fetcher: F,
    ) -> Result<FileMetadata, AppError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<FileMetadata, AppError>>,
    {
        if let Some(cached) = self.get(connection_id, path).await {
            return Ok(cached);
        }

        let key = Self::make_key(connection_id, path);
        let (mut rx, mut leader) = {
            let mut in_flight = self
                .in_flight
                .lock()
                .map_err(|_| AppError::Internal(anyhow::anyhow!("metadata cache in-flight map poisoned")))?;
            if let Some(tx) = in_flight.get(&key) {
                (Some(tx.subscribe()), None)
            } else {
                let (tx, _) = broadcast::channel(1);
                in_flight.insert(key.clone(), tx);
                (
                    None,
                    Some(InFlightLeader {
                        key: key.clone(),
                        map: self.in_flight.clone(),
                        armed: true,
                    }),
                )
            }
        };

        if let Some(ref mut receiver) = rx {
            return match receiver.recv().await {
                Ok(Ok(meta)) => Ok(meta),
                Ok(Err(err_msg)) => Err(AppError::Internal(anyhow::anyhow!(err_msg))),
                Err(_) => self
                    .get(connection_id, path)
                    .await
                    .ok_or_else(|| AppError::NotFound(format!("Path '{}' not found", path))),
            };
        }

        let fetch_res = fetcher().await;
        if let Ok(ref meta) = fetch_res {
            self.put(connection_id, path, meta.clone()).await;
        }
        if let Some(ref mut leader) = leader {
            leader.complete(&fetch_res);
        }
        fetch_res
    }

    pub async fn invalidate(&self, connection_id: &str, path: &str) {
        let key = Self::make_key(connection_id, path);
        let mut entries = self.entries.write().await;
        entries.remove(&key);
    }

    pub async fn invalidate_prefix(&self, connection_id: &str, path_prefix: &str) {
        let exact = Self::make_key(connection_id, path_prefix);
        let descendant_prefix = format!("{}/", exact);
        let mut entries = self.entries.write().await;
        entries.retain(|key, _| key != &exact && !key.starts_with(&descendant_prefix));
    }

    pub async fn clear(&self) {
        let mut entries = self.entries.write().await;
        entries.clear();
    }
}

#[async_trait]
impl FileMetadataCache for MetadataCache {
    async fn get(&self, connection_id: &str, path: &str) -> Option<FileMetadata> {
        MetadataCache::get(self, connection_id, path).await
    }

    async fn put(&self, connection_id: &str, path: &str, metadata: FileMetadata) {
        MetadataCache::put(self, connection_id, path, metadata).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancelling_single_flight_leader_releases_in_flight_slot() {
        let cache = MetadataCache::new(Duration::from_secs(3));
        let task_cache = cache.clone();
        let task = tokio::spawn(async move {
            let _ = task_cache
                .get_or_fetch("local", "/cancelled", || async {
                    std::future::pending::<Result<FileMetadata, AppError>>().await
                })
                .await;
        });

        tokio::task::yield_now().await;
        assert!(cache
            .in_flight
            .lock()
            .unwrap()
            .contains_key("local:/cancelled"));

        task.abort();
        let _ = task.await;
        tokio::task::yield_now().await;

        assert!(!cache
            .in_flight
            .lock()
            .unwrap()
            .contains_key("local:/cancelled"));
    }
}
