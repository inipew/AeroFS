use crate::config::AppConfig;
use crate::db::DbPool;
use crate::infrastructure::CredentialStore;
use crate::services::connection_service::ConnectionService;
use crate::transfer::TransferManager;
use crate::vfs::registry::ProviderRegistry;
use crate::vfs::FileSystem;
use std::sync::Arc;
use tokio::sync::Semaphore;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub db: DbPool,
    pub registry: Arc<ProviderRegistry>,
    pub credentials: Arc<CredentialStore>,
    pub transfer_manager: TransferManager,
    pub global_io_semaphore: Arc<Semaphore>,
    pub archive_semaphore: Arc<Semaphore>,
    pub search_semaphore: Arc<Semaphore>,
}

impl AppState {
    pub async fn new_with_db(config: AppConfig, db: DbPool) -> Self {
        let credentials = Arc::new(CredentialStore::new(&config.security.session_secret));
        let registry = Arc::new(ProviderRegistry::new());
        let transfer_manager = TransferManager::new(
            registry.providers_map(),
            db.clone(),
            config.limits.max_concurrent_transfers,
        );

        let state = Self {
            config: Arc::new(config),
            db,
            registry,
            credentials,
            transfer_manager,
            global_io_semaphore: Arc::new(Semaphore::new(32)),
            archive_semaphore: Arc::new(Semaphore::new(4)),
            search_semaphore: Arc::new(Semaphore::new(8)),
        };

        // Initialize and register all connections from DB via ConnectionService
        ConnectionService::load_all_providers_from_db(&state).await;

        state
    }

    /// Convenience forwarder to ProviderRegistry
    pub async fn get_provider(&self, connection_id: &str) -> Option<Arc<dyn FileSystem>> {
        self.registry.get(connection_id).await
    }

    pub async fn register_provider(&self, connection_id: String, provider: Arc<dyn FileSystem>) {
        self.registry.register(connection_id, provider).await;
    }

    pub async fn remove_provider(&self, connection_id: &str) {
        self.registry.remove(connection_id).await;
    }

    pub async fn set_connection_error(&self, connection_id: &str, error: &str) {
        self.registry
            .set_connection_error(connection_id, error)
            .await;
    }

    pub async fn get_connection_error(&self, connection_id: &str) -> Option<String> {
        self.registry.get_connection_error(connection_id).await
    }

    pub async fn clear_connection_error(&self, connection_id: &str) {
        self.registry.clear_connection_error(connection_id).await;
    }
}
