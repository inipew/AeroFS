use crate::vfs::runtime::StorageRuntime;
use crate::vfs::FileSystem;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionStatus {
    Ready,
    Degraded(String),
    Failed(String),
    Draining,
}

#[derive(Clone)]
pub struct ProviderHandle {
    pub provider: Arc<dyn FileSystem>,
    pub runtime: Arc<StorageRuntime>,
    pub status: ConnectionStatus,
}

#[derive(Default)]
pub struct ProviderRegistry {
    providers: Arc<RwLock<HashMap<String, Arc<dyn FileSystem>>>>,
    runtimes: Arc<RwLock<HashMap<String, Arc<StorageRuntime>>>>,
    connection_errors: Arc<RwLock<HashMap<String, String>>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: Arc::new(RwLock::new(HashMap::new())),
            runtimes: Arc::new(RwLock::new(HashMap::new())),
            connection_errors: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn providers_map(&self) -> Arc<RwLock<HashMap<String, Arc<dyn FileSystem>>>> {
        Arc::clone(&self.providers)
    }

    pub fn runtimes_map(&self) -> Arc<RwLock<HashMap<String, Arc<StorageRuntime>>>> {
        Arc::clone(&self.runtimes)
    }

    pub async fn get(&self, connection_id: &str) -> Option<Arc<dyn FileSystem>> {
        let providers = self.providers.read().await;
        providers.get(connection_id).cloned()
    }

    pub async fn get_runtime(&self, connection_id: &str) -> Option<Arc<StorageRuntime>> {
        let runtimes = self.runtimes.read().await;
        runtimes.get(connection_id).cloned()
    }

    pub async fn get_handle(&self, connection_id: &str) -> Option<ProviderHandle> {
        let providers = self.providers.read().await;
        let runtimes = self.runtimes.read().await;
        let errors = self.connection_errors.read().await;
        let provider = providers.get(connection_id)?.clone();
        let runtime = runtimes.get(connection_id)?.clone();
        let status = if let Some(err) = errors.get(connection_id) {
            ConnectionStatus::Failed(err.clone())
        } else {
            ConnectionStatus::Ready
        };
        Some(ProviderHandle {
            provider,
            runtime,
            status,
        })
    }

    pub async fn register(&self, connection_id: String, provider: Arc<dyn FileSystem>) {
        let runtime = Arc::new(StorageRuntime::new(
            &connection_id,
            Arc::clone(&provider),
            64,
        ));
        let mut providers = self.providers.write().await;
        let mut runtimes = self.runtimes.write().await;
        providers.insert(connection_id.clone(), provider);
        runtimes.insert(connection_id.clone(), runtime);
        self.clear_connection_error(&connection_id).await;
    }

    pub async fn register_runtime(&self, connection_id: String, runtime: Arc<StorageRuntime>) {
        let mut providers = self.providers.write().await;
        let mut runtimes = self.runtimes.write().await;
        providers.insert(connection_id.clone(), Arc::clone(&runtime.provider));
        runtimes.insert(connection_id.clone(), runtime);
        self.clear_connection_error(&connection_id).await;
    }

    pub async fn remove(&self, connection_id: &str) {
        let mut providers = self.providers.write().await;
        let mut runtimes = self.runtimes.write().await;
        providers.remove(connection_id);
        runtimes.remove(connection_id);
        self.clear_connection_error(connection_id).await;
    }

    pub async fn contains(&self, connection_id: &str) -> bool {
        let providers = self.providers.read().await;
        providers.contains_key(connection_id)
    }

    pub async fn list_ids(&self) -> Vec<String> {
        let providers = self.providers.read().await;
        providers.keys().cloned().collect()
    }

    pub async fn set_connection_error(&self, connection_id: &str, error: &str) {
        let mut errors = self.connection_errors.write().await;
        errors.insert(connection_id.to_string(), error.to_string());
    }

    pub async fn get_connection_error(&self, connection_id: &str) -> Option<String> {
        let errors = self.connection_errors.read().await;
        errors.get(connection_id).cloned()
    }

    pub async fn clear_connection_error(&self, connection_id: &str) {
        let mut errors = self.connection_errors.write().await;
        errors.remove(connection_id);
    }

    /// List non-local connections that have 0 leases and have been idle for longer than TTL
    pub async fn get_idle_connections(&self, ttl: std::time::Duration) -> Vec<String> {
        let runtimes = self.runtimes.read().await;
        let mut idle = Vec::new();
        for (id, rt) in runtimes.iter() {
            if id != "local" && rt.is_idle(ttl).await {
                idle.push(id.clone());
            }
        }
        idle
    }
}
