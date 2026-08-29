use crate::domain::FileMetadata;
use crate::errors::AppError;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, Mutex, RwLock};

#[derive(Clone)]
struct CachedMetadata {
    metadata: FileMetadata,
    expires_at: Instant,
}

type InFlightSender = broadcast::Sender<Result<FileMetadata, String>>;
type InFlightMap = Arc<Mutex<HashMap<String, InFlightSender>>>;

/// Thread-safe, bounded, short-TTL metadata cache with Single-Flight request coalescing
/// to completely eliminate cache stampedes on remote storage (S3/SFTP).
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
            in_flight: Arc::new(Mutex::new(HashMap::new())),
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

    /// Single-Flight coalesced fetch: If multiple concurrent callers request the same key
    /// simultaneously during a cache miss, only one I/O fetch is performed and the result
    /// is shared across all concurrent callers.
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
        // 1. Fast path: check cache
        if let Some(cached) = self.get(connection_id, path).await {
            return Ok(cached);
        }

        let key = Self::make_key(connection_id, path);

        // 2. Coalescing check
        let mut rx_opt = None;
        {
            let mut in_flight_map = self.in_flight.lock().await;
            // Check cache again under lock in case it was just populated
            if let Some(cached) = self.get(connection_id, path).await {
                return Ok(cached);
            }

            if let Some(tx) = in_flight_map.get(&key) {
                rx_opt = Some(tx.subscribe());
            } else {
                let (tx, _) = broadcast::channel(1);
                in_flight_map.insert(key.clone(), tx);
            }
        }

        // 3. If another fetch is in-flight, wait on its broadcast channel
        if let Some(mut rx) = rx_opt {
            return match rx.recv().await {
                Ok(Ok(meta)) => Ok(meta),
                Ok(Err(err_msg)) => Err(AppError::Internal(anyhow::anyhow!(err_msg))),
                Err(_) => {
                    // Channel closed or missed; fallback to cache or direct get
                    self.get(connection_id, path)
                        .await
                        .ok_or_else(|| AppError::NotFound(format!("Path '{}' not found", path)))
                }
            };
        }

        // 4. We are the leader: perform the actual I/O fetch
        let fetch_res = fetcher().await;

        // 5. Broadcast to all waiters and store in cache
        let mut in_flight_map = self.in_flight.lock().await;
        if let Some(tx) = in_flight_map.remove(&key) {
            match &fetch_res {
                Ok(meta) => {
                    let _ = tx.send(Ok(meta.clone()));
                }
                Err(e) => {
                    let _ = tx.send(Err(e.to_string()));
                }
            }
        }

        if let Ok(ref meta) = fetch_res {
            self.put(connection_id, path, meta.clone()).await;
        }

        fetch_res
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
