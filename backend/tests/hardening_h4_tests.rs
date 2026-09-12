use std::fs;
use std::path::PathBuf;

fn source(path: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(root.join(path)).expect("source file should be readable")
}

#[test]
fn app_state_exposes_sync_substate() {
    let state = source("src/state.rs");
    assert!(state.contains("pub struct SyncState"));
    assert!(state.contains("FromRef<AppState> for SyncState"));
    assert!(state.contains("pub(crate) sync: SyncState"));
}

#[test]
fn sync_http_uses_only_sync_capability() {
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
        assert!(
            !api.contains(forbidden),
            "sync HTTP adapter leaked dependency: {forbidden}"
        );
    }
}

#[test]
fn sync_service_owns_authorization_and_manager_boundary() {
    let service = source("src/services/sync_service.rs");
    for expected in [
        "authorization: Arc<dyn Authorization>",
        "manager: Arc<SyncManager>",
        "FileAction::Read",
        "FileAction::Write",
        "pub async fn create_job",
        "pub async fn list_jobs",
        "pub async fn list_operations",
        "pub async fn resolve_conflict",
    ] {
        assert!(
            service.contains(expected),
            "sync capability missing dependency or operation: {expected}"
        );
    }

    for forbidden in ["AppState", "check_permission", "DbPool", "ProviderRegistry"] {
        assert!(
            !service.contains(forbidden),
            "sync service leaked concrete dependency: {forbidden}"
        );
    }
}

#[test]
fn bootstrap_composes_sync_capability_once() {
    let bootstrap = source("src/bootstrap.rs");
    assert!(bootstrap.contains("SyncState::new(SyncService::new("));
    assert!(bootstrap.contains("sync_manager.clone()"));
    assert!(bootstrap.contains("sync,"));
}

#[test]
fn sync_manager_remains_runtime_engine_not_http_dependency() {
    let api = source("src/api/sync.rs");
    let manager = source("src/sync/manager.rs");
    assert!(!api.contains("SyncManager"));
    assert!(manager.contains("pub async fn notify_transfer_completed"));
    assert!(manager.contains("pub async fn recover_interrupted_jobs"));
}
