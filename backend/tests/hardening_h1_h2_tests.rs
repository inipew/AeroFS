use std::fs;
use std::path::PathBuf;

fn source(path: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(root.join(path)).expect("source file should be readable")
}

#[test]
fn app_state_exposes_narrow_substate_foundation() {
    let state = source("src/state.rs");
    assert!(state.contains("pub struct SearchState"));
    assert!(state.contains("pub struct HealthState"));
    assert!(state.contains("impl_from_ref!(SearchState, search)"));
    assert!(state.contains("impl_from_ref!(HealthState, health)"));
    assert!(state.contains("pub struct RuntimeView"));
    assert!(!state.contains("pub search_semaphore:"));
}

#[test]
fn search_http_uses_only_search_capability() {
    let api = source("src/api/search.rs");
    assert!(api.contains("State<SearchState>"));
    for forbidden in [
        "State<AppState>",
        "state.db",
        "state.registry",
        "search_semaphore",
        "check_permission(",
    ] {
        assert!(!api.contains(forbidden), "search HTTP adapter leaked dependency: {forbidden}");
    }
}

#[test]
fn search_service_uses_ports_and_owns_its_limiter() {
    let service = source("src/services/search_service.rs");
    for expected in ["Authorization", "FileSystemResolver", "limiter: Arc<Semaphore>"] {
        assert!(service.contains(expected), "search service missing narrow dependency: {expected}");
    }
    for forbidden in ["AppState", "check_permission", "ProviderRegistry", "state.db"] {
        assert!(!service.contains(forbidden), "search service leaked concrete dependency: {forbidden}");
    }
}

#[test]
fn health_http_uses_only_health_capability() {
    let api = source("src/api/health.rs");
    assert!(api.contains("State<HealthState>"));
    for forbidden in [
        "State<AppState>",
        "state.db",
        "state.config",
        "state.registry",
        "state.runtime",
        "sqlx::",
    ] {
        assert!(!api.contains(forbidden), "health HTTP adapter leaked dependency: {forbidden}");
    }
}

#[test]
fn health_service_does_not_accept_app_state() {
    let service = source("src/services/health_service.rs");
    assert!(service.contains("RuntimeView"));
    assert!(service.contains("pub async fn readiness(&self)"));
    assert!(!service.contains("AppState"));
}
