use std::{fs, path::PathBuf};

fn source(path: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(root.join(path)).expect("source file should be readable")
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
        "sqlx::",
    ] {
        assert!(
            !api.contains(forbidden),
            "search HTTP adapter leaked dependency: {forbidden}"
        );
    }
}

#[test]
fn search_service_uses_ports_and_shared_resource_budget() {
    let service = source("src/services/search_service.rs");
    for expected in [
        "Arc<dyn Authorization>",
        "Arc<dyn FileSystemResolver>",
        "budget: Arc<ResourceBudget>",
    ] {
        assert!(
            service.contains(expected),
            "search service missing narrow dependency: {expected}"
        );
    }
    for forbidden in [
        "AppState",
        "check_permission",
        "ProviderRegistry",
        "state.db",
        "limiter: Arc<Semaphore>",
    ] {
        assert!(
            !service.contains(forbidden),
            "search service leaked concrete or independent limiter dependency: {forbidden}"
        );
    }
}

#[test]
fn app_state_exposes_precomposed_search_capability_without_private_limiter() {
    let state = source("src/state.rs");
    assert!(state.contains("pub struct SearchState"));
    assert!(state.contains("impl_from_ref!(SearchState, search)"));
    assert!(!state.contains("pub search_semaphore:"));
}

#[test]
fn bootstrap_composes_search_once_from_shared_file_ports_and_budget() {
    let bootstrap = source("src/bootstrap.rs");
    assert_eq!(bootstrap.matches("SearchState::new(SearchService::new(").count(), 1);
    assert!(bootstrap.contains("file_authorization.clone()"));
    assert!(bootstrap.contains("file_filesystem.clone()"));
    assert!(bootstrap.contains("resource_budget.clone()"));
}
