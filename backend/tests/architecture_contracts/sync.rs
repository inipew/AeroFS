use crate::support::architecture_source as source;

#[test]
fn sync_http_uses_only_sync_capability() {
    let state = source("src/state.rs");
    assert!(state.contains("pub struct SyncState"));
    assert!(state.contains("impl_from_ref!(SyncState, sync)"));
    assert!(state.contains("pub(crate) sync: SyncState"));

    let api = source("src/api/sync.rs");
    assert!(api.contains("State<SyncState>"));
    for forbidden in [
        "State<AppState>",
        "AppState",
        "state.db",
        "state.sync_manager",
        "check_permission(",
        "PermissionAction",
        "sqlx::",
    ] {
        assert!(!api.contains(forbidden), "sync HTTP adapter leaked {forbidden}");
    }
}

#[test]
fn sync_service_owns_authorization_and_control_boundary() {
    let service = source("src/services/sync_service.rs");
    for expected in [
        "authorization: Arc<dyn Authorization>",
        "control: Arc<dyn SyncControl>",
        "FileAction::Read",
        "FileAction::Write",
        "pub async fn create_job",
        "pub async fn list_jobs",
        "pub async fn list_operations",
        "pub async fn resolve_conflict",
    ] {
        assert!(service.contains(expected), "sync capability missing {expected}");
    }
    for forbidden in [
        "AppState",
        "check_permission",
        "DbPool",
        "ProviderRegistry",
        "SyncManager",
    ] {
        assert!(!service.contains(forbidden), "sync service leaked {forbidden}");
    }
}

#[test]
fn bootstrap_composes_sync_capability_and_manager_stays_behind_control_port() {
    let bootstrap = source("src/bootstrap.rs");
    assert!(bootstrap.contains("SyncState::new(SyncService::new("));
    assert!(bootstrap.contains("sync_manager"));
    assert!(bootstrap.contains("sync:"));

    let api = source("src/api/sync.rs");
    let manager = source("src/sync/manager.rs");
    let adapter = source("src/infrastructure/sync.rs");
    assert!(!api.contains("SyncManager"));
    assert!(manager.contains("pub async fn notify_transfer_completed"));
    assert!(manager.contains("pub async fn recover_interrupted_jobs"));
    assert!(adapter.contains("impl SyncControl for SyncManager"));
}
