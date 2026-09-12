use crate::application::{
    files::FileUseCases, transfers::TransferUseCases, UploadApplicationService,
};
use crate::runtime::TaskSupervisor;
use crate::services::{
    ArchiveService, AuditService, AuthService, ConnectionService, FileApiService, HealthService,
    PreferencesService, RealtimeService, SearchService, SettingsService, ShareService, SyncService,
    TrashService,
};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
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

/// Read-only runtime projection safe to expose to request-facing capabilities.
#[derive(Clone)]
pub struct RuntimeView {
    phase: Arc<AtomicU8>,
}

impl RuntimeView {
    pub fn phase(&self) -> RuntimePhase {
        RuntimePhase::from_u8(self.phase.load(Ordering::Acquire))
    }

    pub fn is_running(&self) -> bool {
        self.phase() == RuntimePhase::Running
    }

    pub fn is_shutting_down(&self) -> bool {
        matches!(
            self.phase(),
            RuntimePhase::ShuttingDown | RuntimePhase::Stopped
        )
    }
}

/// Process-owned runtime control plane. This must stay outside HTTP `AppState`.
#[derive(Clone)]
pub struct RuntimeOwner {
    pub shutdown_token: CancellationToken,
    pub force_shutdown_token: CancellationToken,
    pub supervisor: TaskSupervisor,
    pub task_tracker: TaskTracker,
    phase: Arc<AtomicU8>,
    shutdown_reason: Arc<AtomicU8>,
}

impl Default for RuntimeOwner {
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

impl RuntimeOwner {
    pub fn view(&self) -> RuntimeView {
        RuntimeView {
            phase: self.phase.clone(),
        }
    }

    pub fn phase(&self) -> RuntimePhase {
        RuntimePhase::from_u8(self.phase.load(Ordering::Acquire))
    }

    pub fn set_phase(&self, p: RuntimePhase) {
        tracing::info!("runtime.phase={}", p.as_str());
        self.phase.store(p as u8, Ordering::Release);
    }

    pub fn is_shutting_down(&self) -> bool {
        self.view().is_shutting_down()
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

#[derive(Clone)]
pub struct RuntimeState {
    pub view: RuntimeView,
}

impl RuntimeState {
    pub fn new(view: RuntimeView) -> Self {
        Self { view }
    }

    pub fn is_shutting_down(&self) -> bool {
        self.view.is_shutting_down()
    }
}

#[derive(Clone)]
pub struct RouterState {
    pub is_dev: bool,
    pub allowed_origins: Vec<String>,
}

impl RouterState {
    pub fn new(is_dev: bool, allowed_origins: Vec<String>) -> Self {
        Self {
            is_dev,
            allowed_origins,
        }
    }
}

#[derive(Clone)]
pub struct AuthState {
    pub service: AuthService,
}

impl AuthState {
    pub fn new(service: AuthService) -> Self {
        Self { service }
    }
}

#[derive(Clone)]
pub struct ConnectionState {
    pub service: ConnectionService,
}

impl ConnectionState {
    pub fn new(service: ConnectionService) -> Self {
        Self { service }
    }
}

#[derive(Clone)]
pub struct TransferState {
    pub use_cases: TransferUseCases,
}

impl TransferState {
    pub fn new(use_cases: TransferUseCases) -> Self {
        Self { use_cases }
    }
}

#[derive(Clone)]
pub struct SearchState {
    pub service: SearchService,
}

impl SearchState {
    pub fn new(service: SearchService) -> Self {
        Self { service }
    }
}

#[derive(Clone)]
pub struct HealthState {
    pub service: HealthService,
}

impl HealthState {
    pub fn new(service: HealthService) -> Self {
        Self { service }
    }
}

#[derive(Clone)]
pub struct RealtimeState {
    pub service: RealtimeService,
}

impl RealtimeState {
    pub fn new(service: RealtimeService) -> Self {
        Self { service }
    }
}

#[derive(Clone)]
pub struct SyncState {
    pub service: SyncService,
}

impl SyncState {
    pub fn new(service: SyncService) -> Self {
        Self { service }
    }
}

#[derive(Clone)]
pub struct ArchiveState {
    pub service: ArchiveService,
}

impl ArchiveState {
    pub fn new(service: ArchiveService) -> Self {
        Self { service }
    }
}

#[derive(Clone)]
pub struct SettingsState {
    pub service: SettingsService,
}

impl SettingsState {
    pub fn new(service: SettingsService) -> Self {
        Self { service }
    }
}

#[derive(Clone)]
pub struct AuditState {
    pub service: AuditService,
}

impl AuditState {
    pub fn new(service: AuditService) -> Self {
        Self { service }
    }
}

#[derive(Clone)]
pub struct PreferencesState {
    pub service: PreferencesService,
}

impl PreferencesState {
    pub fn new(service: PreferencesService) -> Self {
        Self { service }
    }
}

#[derive(Clone)]
pub struct ShareState {
    pub service: ShareService,
}

impl ShareState {
    pub fn new(service: ShareService) -> Self {
        Self { service }
    }
}

#[derive(Clone)]
pub struct TrashState {
    pub service: TrashService,
}

impl TrashState {
    pub fn new(service: TrashService) -> Self {
        Self { service }
    }
}

#[derive(Clone)]
pub struct FileApiState {
    pub files: FileUseCases,
    pub uploads: UploadApplicationService,
    pub service: FileApiService,
}

impl FileApiState {
    pub fn new(
        files: FileUseCases,
        uploads: UploadApplicationService,
        service: FileApiService,
    ) -> Self {
        Self {
            files,
            uploads,
            service,
        }
    }
}

/// Capability container handed to request adapters.
/// Concrete infrastructure and process lifecycle ownership stay in bootstrap/services.
#[derive(Clone)]
pub struct AppState {
    pub(crate) router: RouterState,
    pub(crate) runtime: RuntimeState,
    pub(crate) auth: AuthState,
    pub(crate) connections: ConnectionState,
    pub(crate) file_api: FileApiState,
    pub(crate) transfers: TransferState,
    pub(crate) search: SearchState,
    pub(crate) health: HealthState,
    pub(crate) realtime: RealtimeState,
    pub(crate) sync: SyncState,
    pub(crate) archive: ArchiveState,
    pub(crate) settings: SettingsState,
    pub(crate) audit: AuditState,
    pub(crate) preferences: PreferencesState,
    pub(crate) shares: ShareState,
    pub(crate) trash: TrashState,
}

macro_rules! impl_from_ref {
    ($state:ty, $field:ident) => {
        impl axum::extract::FromRef<AppState> for $state {
            fn from_ref(state: &AppState) -> Self {
                state.$field.clone()
            }
        }
    };
}

impl_from_ref!(RouterState, router);
impl_from_ref!(RuntimeState, runtime);
impl_from_ref!(AuthState, auth);
impl_from_ref!(ConnectionState, connections);
impl_from_ref!(FileApiState, file_api);
impl_from_ref!(TransferState, transfers);
impl_from_ref!(SearchState, search);
impl_from_ref!(HealthState, health);
impl_from_ref!(RealtimeState, realtime);
impl_from_ref!(SyncState, sync);
impl_from_ref!(ArchiveState, archive);
impl_from_ref!(SettingsState, settings);
impl_from_ref!(AuditState, audit);
impl_from_ref!(PreferencesState, preferences);
impl_from_ref!(ShareState, shares);
impl_from_ref!(TrashState, trash);
