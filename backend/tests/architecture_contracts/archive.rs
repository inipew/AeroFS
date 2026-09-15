use crate::support::architecture_source as source;

#[test]
fn archive_http_uses_only_archive_capability() {
    let src = source("src/api/archive.rs");
    assert!(src.contains("State<ArchiveState>"));
    for forbidden in [
        "State<AppState>",
        "ArchiveService::compress",
        "ArchiveService::extract",
        "archive_semaphore",
        "state.db",
        "state.transfer_manager",
        "check_permission",
    ] {
        assert!(!src.contains(forbidden), "archive HTTP leaked {forbidden}");
    }
}

#[test]
fn archive_service_owns_ports_and_shared_resource_budget() {
    let src = source("src/services/archive_service.rs");
    for expected in [
        "Arc<dyn Authorization>",
        "Arc<dyn FileSystemResolver>",
        "Arc<dyn ArchiveEffects>",
        "Arc<ResourceBudget>",
    ] {
        assert!(src.contains(expected), "archive service missing {expected}");
    }
    for forbidden in [
        "AppState",
        "AuthenticatedUser",
        "check_permission",
        "PermissionAction",
        "record_audit_log",
        "TransferManager",
        "WsEvent",
        "Arc<Semaphore>",
    ] {
        assert!(!src.contains(forbidden), "archive service leaked {forbidden}");
    }
}

#[test]
fn archive_state_and_bootstrap_use_shared_budget_without_raw_archive_limiter() {
    let state = source("src/state.rs");
    assert!(state.contains("pub struct ArchiveState"));
    assert!(state.contains("pub(crate) archive: ArchiveState"));
    assert!(state.contains("impl_from_ref!(ArchiveState, archive)"));
    assert!(!state.contains("archive_semaphore"));

    let bootstrap = source("src/bootstrap.rs");
    assert_eq!(bootstrap.matches("ArchiveState::new(ArchiveService::new(").count(), 1);
    assert!(bootstrap.contains("SqliteArchiveEffects::new("));
    assert!(bootstrap.contains("ResourceBudget::with_limits("));
    assert!(bootstrap.contains("config.limits.archive_concurrency"));
    assert!(bootstrap.contains("resource_budget.clone()"));
    assert!(!bootstrap.contains("Semaphore::new(cfg_limits_archive)"));
}

#[test]
fn archive_effects_are_isolated_behind_port() {
    let port = source("src/ports/archive.rs");
    let adapter = source("src/infrastructure/archive.rs");
    assert!(port.contains("pub trait ArchiveEffects"));
    assert!(adapter.contains("impl ArchiveEffects for SqliteArchiveEffects"));
    assert!(adapter.contains("ARCHIVE_COMPRESS"));
    assert!(adapter.contains("ARCHIVE_EXTRACT"));
    assert!(adapter.contains("ARCHIVE_EXTRACT_SELECTED"));
}
