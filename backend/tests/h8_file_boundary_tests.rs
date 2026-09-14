use std::fs;

fn source(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

fn compact(src: &str) -> String {
    src.chars().filter(|c| !c.is_whitespace()).collect()
}

#[test]
fn file_http_uses_narrow_capability_state() {
    let src = source("src/api/files.rs");
    let compact = compact(&src);

    assert!(compact.contains("State(state):State<FileApiState>"));
    assert!(
        !compact.contains("State(state):State<AppState>"),
        "file HTTP handlers must not extract AppState"
    );
    assert!(
        !compact.contains("usecrate::state::AppState"),
        "file HTTP module must not import AppState"
    );
}

#[test]
fn file_http_has_no_raw_storage_dependencies() {
    let src = source("src/api/files.rs");
    let compact = compact(&src);

    for forbidden in [
        "state.db",
        "state.config",
        "state.get_provider",
        "SettingsService::get_system_setting",
        "sqlx::",
        "record_audit_log",
        "check_permission",
        "SafePath::resolve",
    ] {
        assert!(
            !compact.contains(forbidden),
            "file HTTP module must not contain raw dependency `{forbidden}`"
        );
    }

    assert!(compact.contains("state.service.chmod_recursive("));
    assert!(compact.contains("state.service.storage_info("));
}

#[test]
fn file_api_service_uses_ports_for_runtime_configuration_storage_and_cache() {
    let src = source("src/services/file_api_service.rs");
    let compact = compact(&src);

    for forbidden in ["DbPool", "AppConfig", "SettingsService", "sqlx::"] {
        assert!(
            !src.contains(forbidden),
            "FileApiService must not depend on concrete runtime dependency `{forbidden}`"
        );
    }
    assert!(!compact.contains("usecrate::services::MetadataCache"));
    assert!(!compact.contains("Arc<MetadataCache>"));

    assert!(compact.contains("file_settings:Arc<dynFileSettings>"));
    assert!(compact.contains("connection_storage:Arc<dynConnectionStorageMetadata>"));
    assert!(compact.contains("metadata_cache:Arc<dynFileMetadataCache>"));
    assert!(compact.contains("self.file_settings.max_editable_size().await?"));
    assert!(compact.contains("self.file_settings.local_root().await?"));
    assert!(compact.contains("self.file_settings.allow_symlinks_outside_root().await?"));
}

#[test]
fn file_helper_services_do_not_depend_on_composition_state() {
    let editor = compact(&source("src/services/editor_service.rs"));
    let editor_port = source("src/ports/editor.rs");
    let adapter = compact(&source("src/editor_state_adapter.rs"));

    for forbidden in [
        "usecrate::state::AppState",
        "usecrate::state::FileApiState",
        "state:&AppState",
        "state:&FileApiState",
    ] {
        assert!(
            !editor.contains(forbidden),
            "editor compatibility service must not depend on composition state via `{forbidden}`"
        );
    }
    assert!(editor.contains("EditorFileAccess"));
    assert!(editor_port.contains("pub trait EditorFileAccess"));
    assert!(adapter.contains("implEditorFileAccessforFileApiState"));
    assert!(
        !std::path::Path::new("src/services/preview_service.rs").exists(),
        "unused PreviewService compatibility facade should stay removed"
    );
    assert!(
        !std::path::Path::new("src/services/operation_service.rs").exists(),
        "unused OperationService compatibility facade should stay removed"
    );
}

#[test]
fn transfer_service_is_not_a_root_state_command_facade() {
    let service = compact(&source("src/services/transfer_service.rs"));
    let api = compact(&source("src/api/transfers.rs"));

    assert!(
        !service.contains("usecrate::state::AppState"),
        "TransferService must not import the root AppState"
    );
    assert!(
        !service.contains("state:&AppState"),
        "TransferService must not proxy request commands through root AppState"
    );
    for obsolete in [
        "pubasyncfncreate_transfer(",
        "pubasyncfnlist_transfers(",
        "pubasyncfncancel_transfer(",
        "pubasyncfnretry_transfer(",
        "pubasyncfndismiss_transfer(",
        "pubasyncfnclear_finished_transfers(",
    ] {
        assert!(
            !service.contains(obsolete),
            "request-facing transfer command facade `{obsolete}` must stay removed"
        );
    }
    assert!(
        api.contains("State(state):State<TransferState>"),
        "transfer HTTP handlers should use the narrow TransferState capability"
    );
    assert!(api.contains("state.use_cases"));
}

#[test]
fn download_audit_is_owned_by_read_use_case() {
    let api = source("src/api/files.rs");
    let read = source("src/application/files/read_file.rs");
    let compact_read = compact(&read);

    assert!(!api.contains("FILE_DOWNLOAD"));
    assert!(read.contains("FileAccessEffects"));
    assert!(compact_read.contains("access_effects:Arc<dynFileAccessEffects>"));
    assert!(read.contains("FILE_DOWNLOAD"));
    assert!(compact_read.contains("self.access_effects.accessed("));
}

#[test]
fn temporary_settings_app_state_seam_is_removed() {
    let settings = source("src/services/settings_service.rs");
    let state = source("src/state.rs");

    assert!(
        !settings.contains("SystemSettingsSource"),
        "H7 compatibility settings seam must be removed"
    );
    assert!(
        !state.contains("SystemSettingsSource"),
        "AppState must no longer act as a settings source"
    );
}

#[test]
fn bootstrap_composes_file_api_boundary() {
    let bootstrap = compact(&source("src/bootstrap.rs"));
    let state = compact(&source("src/state.rs"));

    assert!(bootstrap.contains("FileApiState::new("));
    assert!(bootstrap.contains("FileApiService::new("));
    assert!(bootstrap.contains("SqliteConnectionStorageMetadata::new(db.clone())"));
    assert!(bootstrap.contains("file_settings.clone(),max_upload_size"));
    assert!(state.contains("pubstructFileApiState"));
    assert!(state.contains("impl_from_ref!(FileApiState,file_api)"));
}
