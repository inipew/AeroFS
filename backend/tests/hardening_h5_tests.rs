use std::fs;

fn source(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

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
        assert!(
            !src.contains(forbidden),
            "archive HTTP leaked root/concrete dependency: {forbidden}"
        );
    }
}

#[test]
fn archive_service_owns_ports_and_limiter() {
    let src = source("src/services/archive_service.rs");
    for expected in [
        "Arc<dyn Authorization>",
        "Arc<dyn FileSystemResolver>",
        "Arc<dyn ArchiveEffects>",
        "Arc<Semaphore>",
    ] {
        assert!(
            src.contains(expected),
            "archive service missing boundary: {expected}"
        );
    }
    for forbidden in [
        "AppState",
        "AuthenticatedUser",
        "check_permission",
        "PermissionAction",
        "record_audit_log",
        "TransferManager",
        "WsEvent",
    ] {
        assert!(
            !src.contains(forbidden),
            "archive service leaked concrete dependency: {forbidden}"
        );
    }
}

#[test]
fn app_state_exposes_archive_substate_without_raw_archive_limiter() {
    let src = source("src/state.rs");
    assert!(src.contains("pub struct ArchiveState"));
    assert!(src.contains("pub(crate) archive: ArchiveState"));
    assert!(src.contains("impl_from_ref!(ArchiveState, archive)"));
    assert!(!src.contains("archive_semaphore"));
}

#[test]
fn bootstrap_composes_archive_capability_once() {
    let src = source("src/bootstrap.rs");
    assert_eq!(
        src.matches("ArchiveState::new(ArchiveService::new(")
            .count(),
        1
    );
    assert!(src.contains("SqliteArchiveEffects::new("));
    assert!(src.contains("Semaphore::new(cfg_limits_archive)"));
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
