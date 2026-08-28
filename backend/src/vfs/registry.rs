use crate::vfs::FileSystem;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Default)]
pub struct ProviderRegistry {
    providers: Arc<RwLock<HashMap<String, Arc<dyn FileSystem>>>>,
    connection_errors: Arc<RwLock<HashMap<String, String>>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: Arc::new(RwLock::new(HashMap::new())),
            connection_errors: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn providers_map(&self) -> Arc<RwLock<HashMap<String, Arc<dyn FileSystem>>>> {
        Arc::clone(&self.providers)
    }

    pub async fn get(&self, connection_id: &str) -> Option<Arc<dyn FileSystem>> {
        let providers = self.providers.read().await;
        providers.get(connection_id).cloned()
    }

    pub async fn register(&self, connection_id: String, provider: Arc<dyn FileSystem>) {
        let mut providers = self.providers.write().await;
        providers.insert(connection_id.clone(), provider);
        self.clear_connection_error(&connection_id).await;
    }

    pub async fn remove(&self, connection_id: &str) {
        let mut providers = self.providers.write().await;
        providers.remove(connection_id);
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
}
