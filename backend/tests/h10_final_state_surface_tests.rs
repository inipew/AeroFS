use std::fs;

fn source(relative: &str) -> String {
    fs::read_to_string(relative).unwrap_or_else(|error| panic!("failed to read {relative}: {error}"))
}

#[test]
fn app_state_is_capability_only() {
    let state = source("src/state.rs");
    for forbidden in [
        "pub db:",
        "pub config:",
        "pub registry:",
        "pub metadata_cache:",
        "pub event_journal:",
        "pub transfer_manager:",
        "pub upload_locks:",
        "pub task_tracker:",
        "pub shutdown_token:",
    ] {
        assert!(!state.contains(forbidden), "AppState/runtime leaked concrete field `{forbidden}`");
    }
}

#[test]
fn app_state_has_no_provider_or_runtime_escape_hatches() {
    let state = source("src/state.rs");
    for forbidden in [
        "ProviderRegistry",
        "TransferManager",
        "MetadataCache",
        "EventJournal",
        "UploadLockManager",
        "TaskTracker",
        "CancellationToken",
        "DbPool",
        "AppConfig",
    ] {
        assert!(!state.contains(forbidden), "state layer leaked concrete runtime type `{forbidden}`");
    }
}

#[test]
fn http_adapters_never_extract_root_app_state() {
    let api_files = [
        "src/api/archive.rs",
        "src/api/audit.rs",
        "src/api/auth.rs",
        "src/api/connections.rs",
        "src/api/files.rs",
        "src/api/health.rs",
        "src/api/preferences.rs",
        "src/api/realtime.rs",
        "src/api/search.rs",
        "src/api/settings.rs",
        "src/api/shares.rs",
        "src/api/sync.rs",
        "src/api/transfers.rs",
        "src/api/trash.rs",
        "src/api/ws.rs",
    ];

    for path in api_files {
        let src = source(path);
        assert!(!src.contains("State(state): State<AppState>"), "{path} extracts root AppState");
        assert!(!src.contains("State(app_state): State<AppState>"), "{path} extracts root AppState");
        assert!(!src.contains("State(state): State<Arc<AppState>>"), "{path} extracts Arc<AppState>");
    }
}

#[test]
fn bootstrap_is_the_concrete_composition_root() {
    let bootstrap = source("src/bootstrap.rs");
    let state = source("src/state.rs");

    for expected in [
        "ProviderRegistry::new()",
        "EventJournal::init(",
        "TransferManager::new(",
        "SyncManager::new(",
        "MetadataCache::default()",
        "UploadLockManager::default()",
        "ConnectionState::new(",
        "TransferState::new(",
        "AuthState::new(",
        "ShareState::new(",
        "TrashState::new(",
    ] {
        assert!(bootstrap.contains(expected), "bootstrap missing concrete wiring `{expected}`");
    }

    assert!(!state.contains("ProviderRegistry::new()"));
    assert!(!state.contains("TransferManager::new("));
    assert!(!state.contains("EventJournal::init("));
}

#[test]
fn directory_pagination_uses_bounded_keyset_selection_and_reports_full_count() {
    let src = source("src/application/files/list_directory.rs");

    assert!(src.contains("BinaryHeap::with_capacity(limit.saturating_add(2))"));
    assert!(src.contains("if page.len() > limit.saturating_add(1)"));
    assert!(src.contains("compare_entry_to_key"));
    assert!(src.contains("DirectoryCursor"));
    assert!(src.contains("total_count = total_count.saturating_add(1)"));
    assert!(src.contains("total_count: Some(total_count)"));

    // Numeric offsets are unstable under concurrent directory mutation and malformed
    // cursors must not silently reset pagination to the first page.
    assert!(!src.contains("let page_start ="));
    assert!(!src.contains("{\"offset\":"));
    assert!(!src.contains("unwrap_or(0) as usize"));
}
