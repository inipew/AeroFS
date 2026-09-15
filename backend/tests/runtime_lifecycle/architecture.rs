use std::{fs, path::PathBuf};

fn source(path: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(root.join(path))
        .unwrap_or_else(|error| panic!("failed to read architecture source {path}: {error}"))
}

fn source_exists(path: &str) -> bool {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path).exists()
}

fn app_state_block(source: &str) -> &str {
    let start = source.find("pub struct AppState {").expect("AppState must exist");
    let rest = &source[start..];
    let end = rest.find("\n}").expect("AppState block must close") + 2;
    &rest[..end]
}

#[test]
fn app_state_remains_a_private_capability_surface_without_infrastructure_escape_hatches() {
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
        "RuntimeOwner",
    ] {
        assert!(
            !app_state.contains(forbidden),
            "AppState leaked raw infrastructure/ownership type {forbidden}:\n{app_state}"
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
        assert!(app_state.contains(expected), "missing capability state {expected}");
    }

    assert!(
        !app_state
            .lines()
            .skip(1)
            .any(|line| line.trim_start().starts_with("pub ")),
        "AppState fields must remain private"
    );

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
fn bootstrap_owns_concrete_composition_and_returns_runtime_ownership_separately() {
    let bootstrap = source("src/bootstrap.rs");
    let state = source("src/state.rs");

    assert!(bootstrap.contains("pub struct BuiltApplication"));
    assert!(bootstrap.contains("pub state: AppState"));
    assert!(bootstrap.contains("pub runtime: RuntimeOwner"));
    assert!(bootstrap.contains("RuntimeState::new(runtime.view())"));

    for expected in [
        "ProviderRegistry::new()",
        "EventJournal::init(",
        "TransferManager::new(",
        "SyncManager::new(",
        "MetadataCache::default()",
        "UploadLockManager::default()",
        "SqliteAuthorization",
        "RegistryFileSystemResolver",
        "SqliteFileMutationEffects",
        "SqliteFileSettings",
        "SqliteTransferControl",
        "SqliteTransferEffects",
        "SyncEventSubscriber::spawn",
        "MetadataCacheEventSubscriber::spawn",
        "UploadApplicationService::new",
    ] {
        assert!(bootstrap.contains(expected), "bootstrap missing concrete wiring {expected}");
    }

    for forbidden in [
        "ProviderRegistry::new()",
        "TransferManager::new(",
        "EventJournal::init(",
        "SqliteAuthorization",
        "RegistryFileSystemResolver",
        "SqliteFileMutationEffects",
        "SqliteFileSettings",
        "SqliteTransferControl",
        "SqliteTransferEffects",
        "SyncEventSubscriber::spawn",
        "MetadataCacheEventSubscriber::spawn",
        "new_with_db",
    ] {
        assert!(
            !state.contains(forbidden),
            "state.rs must not compose concrete dependency {forbidden}"
        );
    }
}

#[test]
fn http_and_router_layers_cannot_reach_root_runtime_or_infrastructure_ownership() {
    let api_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/api");
    for entry in fs::read_dir(api_root).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let src = fs::read_to_string(&path).unwrap();
        for forbidden in [
            "State<AppState>",
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
                "{} leaked root state dependency {forbidden}",
                path.display()
            );
        }
    }

    let router = source("src/router.rs");
    for forbidden in ["RuntimeOwner", "force_shutdown_token", "task_tracker", ".supervisor"] {
        assert!(
            !router.contains(forbidden),
            "router leaked process runtime ownership handle {forbidden}"
        );
    }
}

#[test]
fn application_boundary_remains_transport_and_storage_adapter_free() {
    let files = source("src/application/files/mod.rs");
    let upload = source("src/application/upload.rs");
    let transfers = source("src/application/transfers.rs");
    let combined = format!("{files}\n{upload}\n{transfers}");

    for forbidden in [
        "AppState",
        "axum::",
        "sqlx::",
        "ProviderRegistry",
        "SqliteAuthorization",
        "RegistryFileSystemResolver",
        "FileApplicationService",
    ] {
        assert!(
            !combined.contains(forbidden),
            "application boundary leaked forbidden dependency {forbidden}"
        );
    }

    let application_mod = source("src/application/mod.rs");
    let services_mod = source("src/services/mod.rs");
    assert!(!application_mod.contains("FileApplicationService"));
    assert!(!files.contains("FileApplicationService"));
    assert!(!source_exists("src/services/file_service.rs"));
    assert!(!services_mod.contains("pub mod file_service"));
    assert!(!services_mod.contains("pub use file_service::FileService"));
}

#[test]
fn cli_retains_runtime_owner_and_server_startup_delegates_to_bootstrap() {
    let context = source("src/cli/context.rs");
    assert!(context.contains("pub struct CliState"));
    assert!(context.contains("runtime: RuntimeOwner"));
    assert!(context.contains("impl Drop for CliState"));
    assert!(context.contains("request_shutdown(ShutdownReason::Manual)"));
    assert!(context.contains("build_application(self.config.clone(), pool).await"));
    assert!(!context.contains("AppState::new_with_db"));

    let serve = source("src/cli/commands/serve.rs");
    assert!(serve.contains("build_application(config, db).await"));
    assert!(serve.contains("let runtime = built.runtime"));
    assert!(serve.contains("shutdown_signal(runtime: RuntimeOwner)"));
    assert!(!serve.contains("state.runtime"));
    assert!(!serve.contains("SqliteAuthorization"));
    assert!(!serve.contains("SyncEventSubscriber"));
}
