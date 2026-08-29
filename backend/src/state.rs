use crate::config::AppConfig;
use crate::db::DbPool;
use crate::infrastructure::CredentialStore;
use crate::services::connection_service::ConnectionService;
use crate::transfer::TransferManager;
use crate::vfs::registry::ProviderRegistry;
use crate::vfs::FileSystem;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

/// Application-wide runtime context managing cancellation and background task tracking
#[derive(Clone)]
pub struct AppRuntime {
    pub shutdown_token: CancellationToken,
    pub task_tracker: TaskTracker,
}

impl Default for AppRuntime {
    fn default() -> Self {
        Self {
            shutdown_token: CancellationToken::new(),
            task_tracker: TaskTracker::new(),
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub db: DbPool,
    pub registry: Arc<ProviderRegistry>,
    pub credentials: Arc<CredentialStore>,
    pub transfer_manager: TransferManager,
    pub metadata_cache: Arc<crate::services::MetadataCache>,
    pub upload_locks: Arc<crate::services::UploadLockManager>,
    pub global_io_semaphore: Arc<Semaphore>,
    pub archive_semaphore: Arc<Semaphore>,
    pub search_semaphore: Arc<Semaphore>,
    pub runtime: AppRuntime,
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
        let metadata_cache = Arc::new(crate::services::MetadataCache::default());
        let upload_locks = Arc::new(crate::services::UploadLockManager::default());
        let runtime = AppRuntime::default();

        let state = Self {
            config: Arc::new(config),
            db,
            registry,
            credentials,
            transfer_manager,
            metadata_cache,
            upload_locks,
            global_io_semaphore: Arc::new(Semaphore::new(32)),
            archive_semaphore: Arc::new(Semaphore::new(4)),
            search_semaphore: Arc::new(Semaphore::new(8)),
            runtime,
        };

        // Initialize and register all connections from DB via ConnectionService
        ConnectionService::load_all_providers_from_db(&state).await;

        // Spawn background cleanup for stale orphan .part files (> 24 hours old) tracked via TaskTracker
        let local_root_clone = state.config.filesystem.default_local_root.clone();
        let cleanup_token = state.runtime.shutdown_token.clone();
        state.runtime.task_tracker.spawn(async move {
            tokio::select! {
                _ = cleanup_token.cancelled() => {
                    tracing::debug!("Stale staging cleanup cancelled by shutdown");
                }
                _ = crate::vfs::cleanup_stale_staging_files(
                    &local_root_clone,
                    std::time::Duration::from_secs(24 * 3600),
                ) => {}
            }
        });

        state
    }

    /// Convenience forwarder to ProviderRegistry with fail-safe local fallback
    pub async fn get_provider(&self, connection_id: &str) -> Option<Arc<dyn FileSystem>> {
        if let Some(p) = self.registry.get(connection_id).await {
            return Some(p);
        }
        if connection_id == "local" {
            let local_root = self.config.filesystem.default_local_root.clone();
            let _ = tokio::fs::create_dir_all(&local_root).await;
            let local_cfg = self.config.storage.get_provider_config("local");
            if let Ok(local_fs) = crate::vfs::factory::ProviderFactory::build_local_with_config(
                "local",
                local_root,
                Some(&local_cfg),
            ) {
                self.registry
                    .register("local".to_string(), local_fs.clone())
                    .await;
                return Some(local_fs);
            }
        }
        None
    }

    /// Retrieve the unified StorageRuntime for a connection (with fail-safe local fallback)
    pub async fn get_storage_runtime(
        &self,
        connection_id: &str,
    ) -> Option<Arc<crate::vfs::runtime::StorageRuntime>> {
        if let Some(rt) = self.registry.get_runtime(connection_id).await {
            return Some(rt);
        }
        // Ensure provider is initialized if local
        if self.get_provider(connection_id).await.is_some() {
            return self.registry.get_runtime(connection_id).await;
        }
        None
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
