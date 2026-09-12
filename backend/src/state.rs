use crate::application::{files::FileUseCases, transfers::TransferUseCases};
use crate::config::AppConfig;
use crate::db::DbPool;
use crate::events::EventJournal;
use crate::infrastructure::CredentialStore;
use crate::runtime::{ResourceBudget, TaskSupervisor};
use crate::sync::SyncManager;
use crate::transfer::{TransferEngine, TransferManager};
use crate::vfs::registry::ProviderRegistry;
use crate::vfs::FileSystem;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

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
        matches!(self.phase(), RuntimePhase::ShuttingDown | RuntimePhase::Stopped)
    }

    pub fn shutdown_reason(&self) -> Option<ShutdownReason> {
        ShutdownReason::from_u8(self.shutdown_reason.load(Ordering::Acquire))
    }

    pub fn request_shutdown(&self, reason: ShutdownReason) -> bool {
        if self
            .shutdown_reason
            .compare_exchange(0, reason as u8, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            tracing::info!("runtime.shutdown_requested: reason={}", reason.as_str());
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

/// Runtime container handed to adapters.
///
/// Concrete dependency construction belongs in `crate::bootstrap`.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub db: DbPool,
    pub registry: Arc<ProviderRegistry>,
    pub credentials: Arc<CredentialStore>,
    pub transfer_manager: TransferManager,
    pub transfer_engine: TransferEngine,
    pub metadata_cache: Arc<crate::services::MetadataCache>,
    pub upload_locks: Arc<crate::services::UploadLockManager>,
    pub global_io_semaphore: Arc<Semaphore>,
    pub archive_semaphore: Arc<Semaphore>,
    pub search_semaphore: Arc<Semaphore>,
    pub resource_budget: Arc<ResourceBudget>,
    pub event_journal: Arc<EventJournal>,
    pub sync_manager: Arc<SyncManager>,
    pub runtime: AppRuntime,
    pub files: FileUseCases,
    pub transfers: TransferUseCases,
}

impl AppState {
    /// Compatibility constructor for tests and non-server callers.
    /// Production startup should use `bootstrap::build_app_state` directly.
    pub async fn new_with_db(config: AppConfig, db: DbPool) -> Self {
        crate::bootstrap::build_app_state(config, db).await
    }

    pub async fn get_provider(&self, connection_id: &str) -> Option<Arc<dyn FileSystem>> {
        self.registry.get(connection_id).await
    }

    pub async fn get_provider_result(
        &self,
        connection_id: &str,
    ) -> Result<Arc<dyn FileSystem>, crate::errors::VfsError> {
        if let Some(p) = self.registry.get(connection_id).await {
            return Ok(p);
        }
        if connection_id == crate::domain::ConnectionId::LOCAL {
            return Err(crate::errors::VfsError::ConnectionError(
                "Local provider not initialized; call ensure_provider".into(),
            ));
        }
        Err(crate::errors::VfsError::ConnectionError(format!(
            "Connection '{}' not found or provider not initialized",
            connection_id
        )))
    }

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

    pub async fn get_storage_runtime(
        &self,
        connection_id: &str,
    ) -> Option<Arc<crate::vfs::runtime::StorageRuntime>> {
        if let Some(rt) = self.registry.get_runtime(connection_id).await {
            return Some(rt);
        }
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
        self.registry.set_connection_error(connection_id, error).await;
    }

    pub async fn get_connection_error(&self, connection_id: &str) -> Option<String> {
        self.registry.get_connection_error(connection_id).await
    }

    pub async fn clear_connection_error(&self, connection_id: &str) {
        self.registry.clear_connection_error(connection_id).await;
    }
}
