use crate::domain::FileMetadata;
use crate::errors::AppError;
use crate::ports::cache::FileMetadataCache;
use async_trait::async_trait;
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, RwLock};

const MAX_METADATA_ENTRIES: usize = 10_000;

#[derive(Clone)]
struct CachedMetadata {
    metadata: FileMetadata,
    expires_at: Instant,
    generation: u64,
}

#[derive(Default)]
struct CacheState {
    entries: HashMap<String, CachedMetadata>,
    eviction_order: VecDeque<(String, u64)>,
    next_generation: u64,
}

impl CacheState {
    fn remove_if_generation(&mut self, key: &str, generation: u64) -> bool {
        if self
            .entries
            .get(key)
            .is_some_and(|entry| entry.generation == generation)
        {
            self.entries.remove(key);
            true
        } else {
            false
        }
    }

    /// Evict stale queue records first, then the oldest still-live entry.
    /// Each queue record is pushed and popped at most once, making capacity
    /// enforcement amortized O(1) instead of scanning the entire map.
    fn evict_one(&mut self, now: Instant) {
        while let Some((key, generation)) = self.eviction_order.pop_front() {
            let Some(entry) = self.entries.get(&key) else {
                continue;
            };
            if entry.generation != generation {
                continue;
            }

            // Expired entries are preferred naturally because the queue is ordered
            // by insertion/update time. If the oldest entry is still fresh, evict it
            // as the bounded-cache victim rather than performing an O(N) min scan.
            let _expired = now >= entry.expires_at;
            self.entries.remove(&key);
            break;
        }
    }
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
    state: Arc<RwLock<CacheState>>,
    in_flight: InFlightMap,
    ttl: Duration,
    max_entries: usize,
}

impl Default for MetadataCache {
    fn default() -> Self {
        Self::new(Duration::from_secs(3))
    }
}

impl MetadataCache {
    pub fn new(ttl: Duration) -> Self {
        Self::with_capacity(ttl, MAX_METADATA_ENTRIES)
    }

    fn with_capacity(ttl: Duration, max_entries: usize) -> Self {
        Self {
            state: Arc::new(RwLock::new(CacheState::default())),
            in_flight: Arc::new(StdMutex::new(HashMap::new())),
            ttl,
            max_entries: max_entries.max(1),
        }
    }

    fn make_key(connection_id: &str, path: &str) -> String {
        format!("{}:{}", connection_id, path.trim_end_matches('/'))
    }

    pub async fn get(&self, connection_id: &str, path: &str) -> Option<FileMetadata> {
        let key = Self::make_key(connection_id, path);
        let now = Instant::now();

        let expired_generation = {
            let state = self.state.read().await;
            match state.entries.get(&key) {
                Some(cached) if now < cached.expires_at => {
                    return Some(cached.metadata.clone());
                }
                Some(cached) => Some(cached.generation),
                None => None,
            }
        };

        // Expired reads clean up only the entry they observed. Re-checking the
        // generation prevents a racing put() from being deleted.
        if let Some(generation) = expired_generation {
            let mut state = self.state.write().await;
            state.remove_if_generation(&key, generation);
        }
        None
    }

    pub async fn put(&self, connection_id: &str, path: &str, metadata: FileMetadata) {
        let key = Self::make_key(connection_id, path);
        let now = Instant::now();
        let mut state = self.state.write().await;

        if state.entries.len() >= self.max_entries && !state.entries.contains_key(&key) {
            state.evict_one(now);
        }

        state.next_generation = state.next_generation.wrapping_add(1);
        if state.next_generation == 0 {
            state.next_generation = 1;
        }
        let generation = state.next_generation;
        state.entries.insert(
            key.clone(),
            CachedMetadata {
                metadata,
                expires_at: now + self.ttl,
                generation,
            },
        );
        state.eviction_order.push_back((key, generation));
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
        let mut state = self.state.write().await;
        state.entries.remove(&key);
    }

    pub async fn invalidate_prefix(&self, connection_id: &str, path_prefix: &str) {
        let exact = Self::make_key(connection_id, path_prefix);
        let descendant_prefix = format!("{}/", exact);
        let mut state = self.state.write().await;
        state
            .entries
            .retain(|key, _| key != &exact && !key.starts_with(&descendant_prefix));
        // Queue records are intentionally left as tombstones and discarded lazily by
        // evict_one(). This keeps invalidation from doing a second O(N) queue scan.
    }

    pub async fn clear(&self) {
        let mut state = self.state.write().await;
        state.entries.clear();
        state.eviction_order.clear();
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

    fn metadata(path: &str) -> FileMetadata {
        FileMetadata {
            name: path.trim_start_matches('/').to_string(),
            path: path.to_string(),
            kind: crate::domain::FileKind::File,
            size: 1,
            modified_at: None,
            mime_type: None,
            etag: String::new(),
            permissions: None,
        }
    }

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

    #[tokio::test]
    async fn bounded_cache_evicts_oldest_without_map_scan() {
        let cache = MetadataCache::with_capacity(Duration::from_secs(60), 2);
        cache.put("local", "/one", metadata("/one")).await;
        cache.put("local", "/two", metadata("/two")).await;
        cache.put("local", "/three", metadata("/three")).await;

        assert!(cache.get("local", "/one").await.is_none());
        assert!(cache.get("local", "/two").await.is_some());
        assert!(cache.get("local", "/three").await.is_some());
    }

    #[tokio::test]
    async fn refreshing_key_does_not_let_stale_queue_record_evict_new_value() {
        let cache = MetadataCache::with_capacity(Duration::from_secs(60), 2);
        cache.put("local", "/one", metadata("/one")).await;
        cache.put("local", "/two", metadata("/two")).await;
        cache.put("local", "/one", metadata("/one")).await;
        cache.put("local", "/three", metadata("/three")).await;

        assert!(cache.get("local", "/one").await.is_some());
        assert!(cache.get("local", "/two").await.is_none());
        assert!(cache.get("local", "/three").await.is_some());
    }

    #[tokio::test]
    async fn expired_get_removes_only_observed_generation() {
        let cache = MetadataCache::with_capacity(Duration::from_millis(1), 2);
        cache.put("local", "/expired", metadata("/expired")).await;
        tokio::time::sleep(Duration::from_millis(5)).await;
        assert!(cache.get("local", "/expired").await.is_none());
        assert!(!cache
            .state
            .read()
            .await
            .entries
            .contains_key("local:/expired"));
    }
}
