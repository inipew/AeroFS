use std::{fs, path::PathBuf};

fn source(path: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(root.join(path)).unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

fn app_state_block(src: &str) -> &str {
    let start = src.find("pub struct AppState {").expect("AppState must exist");
    let rest = &src[start..];
    let end = rest.find("\n}").expect("AppState block must close") + 2;
    &rest[..end]
}

#[test]
fn app_state_is_capability_only() {
    let state = source("src/state.rs");
    let app_state = app_state_block(&state);

    for forbidden in [
        "DbPool",
        "AppConfig",
        "ProviderRegistry",
        "CredentialStore",
        "TransferManager",
        "TransferEngine",
        "MetadataCache",
        "UploadLockManager",
        "Semaphore",
        "ResourceBudget",
        "EventJournal",
        "SyncManager",
    ] {
        assert!(
            !app_state.contains(forbidden),
            "AppState leaked raw infrastructure type {forbidden}:\n{app_state}"
        );
    }

    for expected in [
        "RouterState",
        "RuntimeState",
        "AuthState",
        "ConnectionState",
        "FileApiState",
        "TransferState",
        "SearchState",
        "HealthState",
        "RealtimeState",
        "SyncState",
        "ArchiveState",
        "SettingsState",
        "AuditState",
        "PreferencesState",
        "ShareState",
        "TrashState",
    ] {
        assert!(app_state.contains(expected), "missing capability {expected}");
    }

    assert!(
        !app_state
            .lines()
            .skip(1)
            .any(|line| line.trim_start().starts_with("pub ")),
        "AppState fields must not be publicly exposed"
    );
}

#[test]
fn app_state_has_no_provider_or_runtime_escape_hatches() {
    let state = source("src/state.rs");
    for forbidden in [
        "fn get_provider(",
        "fn get_storage_runtime(",
        "fn get_provider_handle(",
        "fn list_provider_ids(",
        "fn db(",
        "fn registry(",
        "fn transfer_manager(",
        "fn event_journal(",
    ] {
        assert!(!state.contains(forbidden), "state escape hatch returned: {forbidden}");
    }
}

#[test]
fn http_adapters_never_extract_root_app_state() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/api");
    for entry in fs::read_dir(root).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let src = fs::read_to_string(&path).unwrap();
        for forbidden in [
            "State<AppState>",
            "State(state): State<AppState>",
            "state.db",
            "state.registry",
            "state.credentials",
            "state.config",
            "state.transfer_manager",
            "state.transfer_engine",
            "state.metadata_cache",
            "state.upload_locks",
            "state.event_journal",
            "state.sync_manager",
        ] {
            assert!(
                !src.contains(forbidden),
                "{} leaked root state dependency `{forbidden}`",
                path.display()
            );
        }
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

    assert!(!src.contains("let page_start ="));
    assert!(!src.contains("unwrap_or(0) as usize"));
}
