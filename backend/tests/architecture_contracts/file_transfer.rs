use crate::support::{
    architecture_async_function as async_function, architecture_source as source,
    compact_source as compact, source_exists,
};

#[test]
fn file_http_uses_narrow_capability_without_raw_storage_or_compatibility_dependencies() {
    let src = source("src/api/files.rs");
    let compact = compact(&src);

    assert!(compact.contains("State(state):State<FileApiState>"));
    assert!(!compact.contains("State(state):State<AppState>"));
    assert!(!compact.contains("usecrate::state::AppState"));
    for forbidden in [
        "state.db",
        "state.config",
        "state.get_provider",
        "SettingsService::get_system_setting",
        "sqlx::",
        "record_audit_log",
        "check_permission",
        "SafePath::resolve",
        "FileApplicationService::from_state",
        "FileApplicationService::new",
    ] {
        assert!(!compact.contains(forbidden), "file HTTP leaked {forbidden}");
    }
    for expected in [
        "state.service.chmod_recursive(",
        "state.service.storage_info(",
        ".write_file",
        ".presign_download",
        ".chmod_entry",
        ".copy_entry",
    ] {
        assert!(compact.contains(expected), "file HTTP missing boundary call {expected}");
    }
}

#[test]
fn file_copy_handler_delegates_orchestration_without_raw_copy_effects() {
    let src = source("src/api/files.rs");
    let handler = async_function(&src, "copy_entry");

    assert!(handler.contains(".copy_entry"));
    for forbidden in ["provider.copy", "record_audit_log", "metadata_cache", "event_journal"] {
        assert!(
            !handler.contains(forbidden),
            "copy handler leaked orchestration dependency {forbidden}"
        );
    }
}

#[test]
fn file_api_service_uses_ports_for_settings_storage_and_cache() {
    let src = source("src/services/file_api_service.rs");
    let compact = compact(&src);
    for forbidden in ["DbPool", "AppConfig", "SettingsService", "sqlx::"] {
        assert!(!src.contains(forbidden), "FileApiService leaked {forbidden}");
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
fn file_helper_services_depend_on_ports_not_composition_state() {
    let editor = compact(&source("src/services/editor_service.rs"));
    let editor_port = source("src/ports/editor.rs");
    let adapter = compact(&source("src/editor_state_adapter.rs"));

    for forbidden in [
        "usecrate::state::AppState",
        "usecrate::state::FileApiState",
        "state:&AppState",
        "state:&FileApiState",
    ] {
        assert!(!editor.contains(forbidden), "editor service leaked {forbidden}");
    }
    assert!(editor.contains("EditorFileAccess"));
    assert!(editor_port.contains("pub trait EditorFileAccess"));
    assert!(adapter.contains("implEditorFileAccessforFileApiState"));
    assert!(!source_exists("src/services/preview_service.rs"));
    assert!(!source_exists("src/services/operation_service.rs"));
}

#[test]
fn transfer_http_uses_application_boundary_and_legacy_command_facade_stays_removed() {
    let service = compact(&source("src/services/transfer_service.rs"));
    let api = compact(&source("src/api/transfers.rs"));

    assert!(api.contains("State(state):State<TransferState>"));
    assert!(api.contains("state.use_cases"));
    for forbidden in [
        "State(state):State<AppState>",
        "TransferService",
        "state.transfer_manager",
        "check_permission(",
    ] {
        assert!(!api.contains(forbidden), "transfer HTTP leaked {forbidden}");
    }

    assert!(!service.contains("usecrate::state::AppState"));
    assert!(!service.contains("state:&AppState"));
    for obsolete in [
        "pubasyncfncreate_transfer(",
        "pubasyncfnlist_transfers(",
        "pubasyncfncancel_transfer(",
        "pubasyncfnretry_transfer(",
        "pubasyncfndismiss_transfer(",
        "pubasyncfnclear_finished_transfers(",
    ] {
        assert!(!service.contains(obsolete), "legacy transfer facade returned: {obsolete}");
    }
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
fn temporary_settings_seam_stays_removed_and_bootstrap_composes_file_boundary() {
    let settings = source("src/services/settings_service.rs");
    let state = source("src/state.rs");
    assert!(!settings.contains("SystemSettingsSource"));
    assert!(!state.contains("SystemSettingsSource"));

    let bootstrap = compact(&source("src/bootstrap.rs"));
    let compact_state = compact(&state);
    assert!(bootstrap.contains("FileApiState::new("));
    assert!(bootstrap.contains("FileApiService::new("));
    assert!(bootstrap.contains("SqliteConnectionStorageMetadata::new(db.clone())"));
    assert!(bootstrap.contains("file_settings.clone(),max_upload_size"));
    assert!(compact_state.contains("pubstructFileApiState"));
    assert!(compact_state.contains("impl_from_ref!(FileApiState,file_api)"));
}
