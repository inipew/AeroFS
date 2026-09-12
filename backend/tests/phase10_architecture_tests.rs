use std::fs;
use std::path::PathBuf;

fn source(path: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(root.join(path)).expect("source file should be readable")
}

#[test]
fn app_state_is_not_the_composition_root() {
    let state = source("src/state.rs");

    for forbidden in [
        "SqliteAuthorization",
        "RegistryFileSystemResolver",
        "SqliteFileMutationEffects",
        "SqliteFileSettings",
        "SqliteTransferControl",
        "SqliteTransferEffects",
        "SyncEventSubscriber::spawn",
        "MetadataCacheEventSubscriber::spawn",
    ] {
        assert!(
            !state.contains(forbidden),
            "state.rs must not compose concrete dependency: {forbidden}"
        );
    }
    assert!(state.contains("crate::bootstrap::build_app_state"));
}

#[test]
fn bootstrap_owns_concrete_wiring() {
    let bootstrap = source("src/bootstrap.rs");
    for expected in [
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
        assert!(
            bootstrap.contains(expected),
            "bootstrap.rs should own wiring for {expected}"
        );
    }
}

#[test]
fn application_layer_does_not_depend_on_http_state_or_concrete_storage() {
    let files_mod = source("src/application/files/mod.rs");
    let upload = source("src/application/upload.rs");
    let transfers = source("src/application/transfers.rs");
    let combined = format!("{files_mod}\n{upload}\n{transfers}");

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
            "application boundary leaked forbidden dependency: {forbidden}"
        );
    }
}

#[test]
fn server_startup_delegates_to_bootstrap() {
    let serve = source("src/cli/commands/serve.rs");
    assert!(serve.contains("build_application(config, db).await"));
    assert!(serve.contains("let runtime = built.runtime"));
    assert!(!serve.contains("state.runtime"));
    assert!(!serve.contains("SqliteAuthorization"));
    assert!(!serve.contains("SyncEventSubscriber"));
    assert!(!serve.contains("DELETE FROM sessions"));
}

#[test]
fn removed_file_application_facade_cannot_return() {
    let application_mod = source("src/application/mod.rs");
    let files_mod = source("src/application/files/mod.rs");
    let file_service = source("src/services/file_service.rs");
    assert!(!application_mod.contains("FileApplicationService"));
    assert!(!files_mod.contains("FileApplicationService"));
    assert!(!file_service.contains("FileApplicationService"));
}
