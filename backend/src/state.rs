use crate::config::AppConfig;
use crate::db::DbPool;
use crate::events::EventJournal;
use crate::infrastructure::CredentialStore;
use crate::runtime::{ResourceBudget, TaskSupervisor};
use crate::services::connection_service::ConnectionService;
use crate::sync::SyncManager;
use crate::transfer::TransferManager;
use crate::vfs::registry::ProviderRegistry;
use crate::vfs::FileSystem;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

/// Lifecycle phase of the runtime
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RuntimePhase {
    Starting = 0,
    Binding = 1,
    Running = 2,
    ShuttingDown = 3,
    Stopped = 4,
}

impl RuntimePhase {
    fn from_u8(v: u8) -> Self {
        match v {
            0 => RuntimePhase::Starting,
            1 => RuntimePhase::Binding,
            2 => RuntimePhase::Running,
            3 => RuntimePhase::ShuttingDown,
            4 => RuntimePhase::Stopped,
            _ => RuntimePhase::Stopped,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            RuntimePhase::Starting => "starting",
            RuntimePhase::Binding => "binding",
            RuntimePhase::Running => "running",
            RuntimePhase::ShuttingDown => "shutting_down",
            RuntimePhase::Stopped => "stopped",
        }
    }
}

/// The reason why shutdown was initiated
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ShutdownReason {
    CtrlC = 1,
    Sigterm = 2,
    Internal = 3,
    Manual = 4,
}

impl ShutdownReason {
    fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(ShutdownReason::CtrlC),
            2 => Some(ShutdownReason::Sigterm),
            3 => Some(ShutdownReason::Internal),
            4 => Some(ShutdownReason::Manual),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ShutdownReason::CtrlC => "ctrl_c",
            ShutdownReason::Sigterm => "sigterm",
            ShutdownReason::Internal => "internal",
            ShutdownReason::Manual => "manual",
        }
    }
}

/// Application-wide runtime context managing lifecycle phase, cancellation, and background task tracking
#[derive(Clone)]
pub struct AppRuntime {
    pub shutdown_token: CancellationToken,
    pub force_shutdown_token: CancellationToken,
    pub supervisor: TaskSupervisor,
    pub task_tracker: TaskTracker,
    phase: Arc<AtomicU8>,
    shutdown_reason: Arc<AtomicU8>,
}

impl Default for AppRuntime {
    fn default() -> Self {
        let supervisor = TaskSupervisor::new();
        let task_tracker = supervisor.tracker().clone();
        Self {
            shutdown_token: CancellationToken::new(),
            force_shutdown_token: CancellationToken::new(),
            supervisor,
            task_tracker,
            phase: Arc::new(AtomicU8::new(RuntimePhase::Starting as u8)),
            shutdown_reason: Arc::new(AtomicU8::new(0)),
        }
    }
}

impl AppRuntime {
    pub fn phase(&self) -> RuntimePhase {
        RuntimePhase::from_u8(self.phase.load(Ordering::Acquire))
    }

    pub fn set_phase(&self, p: RuntimePhase) {
        tracing::info!("runtime.phase={}", p.as_str());
        self.phase.store(p as u8, Ordering::Release);
    }

    pub fn is_shutting_down(&self) -> bool {
        let p = self.phase();
        p == RuntimePhase::ShuttingDown || p == RuntimePhase::Stopped
    }

    pub fn shutdown_reason(&self) -> Option<ShutdownReason> {
        ShutdownReason::from_u8(self.shutdown_reason.load(Ordering::Acquire))
    }

    pub fn request_shutdown(&self, reason: ShutdownReason) -> bool {
        // Atomic first-wins: only the first caller initiates shutdown and sets the canonical reason
        if self
            .shutdown_reason
            .compare_exchange(0, reason as u8, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            let reason_str = reason.as_str();
            tracing::info!("runtime.shutdown_requested: reason={}", reason_str);
            self.set_phase(RuntimePhase::ShuttingDown);
            self.shutdown_token.cancel();
            true
        } else {
            tracing::debug!(
                "runtime.shutdown_requested ignored: already shutting down with reason={:?}",
                self.shutdown_reason()
            );
            false
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
    pub resource_budget: Arc<ResourceBudget>,
    pub event_journal: Arc<EventJournal>,
    pub sync_manager: Arc<SyncManager>,
    pub runtime: AppRuntime,
}

impl AppState {
    pub async fn new_with_db(config: AppConfig, db: DbPool) -> Self {
        // Key separation (§100): prefer dedicated credential key, fallback to session_secret with warning
        let cred_key = config
            .security
            .credential_encryption_key
            .as_deref()
            .unwrap_or(&config.security.session_secret);
        if config.security.credential_encryption_key.is_none()
            && config.security.session_secret != "dev_secret_change_in_production_32_chars_min"
        {
            tracing::warn!("Using session_secret for credential encryption — set AEROFS_CREDENTIAL_ENCRYPTION_KEY for key separation");
        }
        let credentials = Arc::new(CredentialStore::new(cred_key));
        let registry = Arc::new(ProviderRegistry::new());
        let runtime = AppRuntime::default();

        let event_journal = Arc::new(
            EventJournal::init(db.clone())
                .await
                .expect("Failed to initialize durable event journal"),
        );

        let resource_budget = Arc::new(ResourceBudget::default());

        // TransferManager receives shutdown_token + task_tracker so its internal tasks
        // (recovery + scheduler) are tracked and respond to cancellation.
        // Recovery is awaited synchronously so server only announces readiness after jobs are loaded.
        let transfer_manager = TransferManager::new(
            registry.providers_map(),
            db.clone(),
            config.limits.max_concurrent_transfers,
            event_journal.clone(),
            runtime.shutdown_token.clone(),
            &runtime.task_tracker,
        )
        .await;

        let sync_manager = Arc::new(SyncManager::new(
            db.clone(),
            transfer_manager.clone(),
            runtime.supervisor.clone(),
            event_journal.clone(),
            registry.providers_map(),
        ));

        // Spawn transfer completion listener for sync operations — dual path (§36):
        // Legacy path via TransferManager completion channel (kept for compat) +
        // new event-bus path via EventJournal (decoupled, §121).
        let mut completion_rx = transfer_manager.completion_receiver();
        let sync_manager_clone = sync_manager.clone();
        let shutdown_token_cl = runtime.shutdown_token.clone();
        runtime.supervisor.spawn("sync_completion_listener", async move {
            loop {
                tokio::select! {
                    _ = shutdown_token_cl.cancelled() => break,
                    result = completion_rx.recv() => {
                        match result {
                            Ok((transfer_job_id, success)) => {
                                let _ = sync_manager_clone.notify_transfer_completed(&transfer_job_id, success).await;
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        }
                    }
                }
            }
        });
        // Event-bus based sync subscriber — formalized (§121)
        let mut event_rx = event_journal.subscribe();
        let sync_manager_ev = sync_manager.clone();
        let shutdown_token_ev = runtime.shutdown_token.clone();
        runtime.supervisor.spawn("sync_event_subscriber", async move {
            loop {
                tokio::select! {
                    _ = shutdown_token_ev.cancelled() => break,
                    ev = event_rx.recv() => {
                        match ev {
                            Ok(envelope) => {
                                match envelope.event {
                                    crate::events::DomainEvent::TransferCompleted(ref v) => {
                                        if let Some(id) = v.get("id").and_then(|x| x.as_str()) {
                                            let _ = sync_manager_ev.notify_transfer_completed(id, true).await;
                                        }
                                    }
                                    crate::events::DomainEvent::TransferFailed(ref v) => {
                                        if let Some(id) = v.get("id").and_then(|x| x.as_str()) {
                                            let _ = sync_manager_ev.notify_transfer_completed(id, false).await;
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        }
                    }
                }
            }
        });

        let metadata_cache = Arc::new(crate::services::MetadataCache::default());
        let upload_locks = Arc::new(crate::services::UploadLockManager::default());

        let cfg_limits_global = config.limits.global_io_concurrency;
        let cfg_limits_archive = config.limits.archive_concurrency;
        let cfg_limits_search = config.limits.search_concurrency;
        let state = Self {
            config: Arc::new(config),
            db,
            registry,
            credentials,
            transfer_manager,
            metadata_cache,
            upload_locks,
            global_io_semaphore: Arc::new(Semaphore::new(cfg_limits_global)),
            archive_semaphore: Arc::new(Semaphore::new(cfg_limits_archive)),
            search_semaphore: Arc::new(Semaphore::new(cfg_limits_search)),
            resource_budget,
            event_journal,
            sync_manager,
            runtime,
        };

        // Initialize and register all connections from DB via ConnectionService
        ConnectionService::load_all_providers_from_db(&state).await;

        // Spawn periodic cleanup for stale orphan .part files tracked via TaskSupervisor (config-driven §127)
        let local_root_clone = state.config.filesystem.default_local_root.clone();
        let cleanup_token = state.runtime.shutdown_token.clone();
        state.runtime.supervisor.spawn("stale_staging_cleanup", async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(
                crate::config::EVENT_JOURNAL_VACUUM_SECS,
            ));
            interval.tick().await;
            loop {
                tokio::select! {
                    _ = cleanup_token.cancelled() => {
                        tracing::debug!("Stale staging cleanup cancelled by shutdown");
                        break;
                    }
                    _ = interval.tick() => {
                        let _ = crate::vfs::cleanup_stale_staging_files(
                            &local_root_clone,
                            std::time::Duration::from_secs(crate::config::STAGING_RETENTION_SECS),
                        )
                        .await;
                    }
                }
            }
        });

        // Spawn event journal vacuum task every 6 hours
        let journal_clone = state.event_journal.clone();
        let vacuum_token = state.runtime.shutdown_token.clone();
        state
            .runtime
            .supervisor
            .spawn("event_journal_vacuum", async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(
                    crate::config::EVENT_JOURNAL_VACUUM_SECS,
                ));
                interval.tick().await;
                loop {
                    tokio::select! {
                        _ = vacuum_token.cancelled() => break,
                        _ = interval.tick() => {
                            let _ = journal_clone
                                .vacuum(std::time::Duration::from_secs(
                                    crate::config::EVENT_JOURNAL_RETENTION_SECS,
                                ))
                                .await;
                        }
                    }
                }
            });

        state
    }

    /// Pure read — no FS mutation. Use `ensure_provider` if lazy init of `local` is required (67.md §14).
    pub async fn get_provider(&self, connection_id: &str) -> Option<Arc<dyn FileSystem>> {
        self.registry.get(connection_id).await
    }

    /// Typed variant that preserves failure reason (67.md §15).
    pub async fn get_provider_result(
        &self,
        connection_id: &str,
    ) -> Result<Arc<dyn FileSystem>, crate::errors::VfsError> {
        if let Some(p) = self.registry.get(connection_id).await {
            return Ok(p);
        }
        if connection_id == crate::domain::ConnectionId::LOCAL {
            // Local not yet registered — caller should use ensure_provider
            return Err(crate::errors::VfsError::ConnectionError(
                "Local provider not initialized; call ensure_provider".into(),
            ));
        }
        Err(crate::errors::VfsError::ConnectionError(format!(
            "Connection '{}' not found or provider not initialized",
            connection_id
        )))
    }

    /// Ensure local provider is initialized — isolated side-effect (67.md §14).
    /// Separate from `get_provider` so callers make the mutation explicit.
    pub async fn ensure_provider(&self, connection_id: &str) -> Option<Arc<dyn FileSystem>> {
        if let Some(p) = self.registry.get(connection_id).await {
            return Some(p);
        }
        if connection_id == crate::domain::ConnectionId::LOCAL {
            let local_root = self.config.filesystem.default_local_root.clone();
            if let Err(e) = tokio::fs::create_dir_all(&local_root).await {
                tracing::warn!(
                    "ensure_provider: create_dir_all {:?} failed: {}",
                    local_root,
                    e
                );
            }
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
        if self.ensure_provider(connection_id).await.is_some() {
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
