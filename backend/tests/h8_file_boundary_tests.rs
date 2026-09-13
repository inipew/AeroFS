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
    assert!(!compact.contains("State(state):State<AppState>"));
    assert!(!compact.contains("usecrate::state::AppState"));
}

#[test]
fn file_http_has_no_raw_storage_dependencies() {
    let src = source("src/api/files.rs");
    let compact = compact(&src);
    for forbidden in [
        "state.db", "state.config", "state.get_provider", "SettingsService::get_system_setting",
        "sqlx::", "record_audit_log", "check_permission", "SafePath::resolve",
    ] {
        assert!(!compact.contains(forbidden), "file HTTP module must not contain raw dependency `{forbidden}`");
    }
    assert!(compact.contains("state.service.chmod_recursive("));
    assert!(compact.contains("state.service.storage_info("));
}

#[test]
fn file_api_service_uses_ports_for_runtime_configuration_storage_and_cache() {
    let src = source("src/services/file_api_service.rs");
    let compact = compact(&src);
    for forbidden in ["DbPool", "AppConfig", "SettingsService", "sqlx::"] {
        assert!(!src.contains(forbidden), "FileApiService must not depend on concrete runtime dependency `{forbidden}`");
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
fn metadata_cache_implements_application_cache_port() {
    let port = source("src/ports/cache.rs");
    let cache = compact(&source("src/services/cache.rs"));
    assert!(port.contains("pub trait FileMetadataCache"));
    assert!(cache.contains("implFileMetadataCacheforMetadataCache"));
}

#[test]
fn sqlite_file_settings_propagate_database_failures() {
    let compact = compact(&source("src/infrastructure/files.rs"));
    assert!(compact.contains("asyncfnsetting(&self,key:&str)->Result<Option<String>,AppError>"));
    assert!(compact.contains(".fetch_optional(&self.db).await.map_err("));
    assert!(!compact.contains(".fetch_optional(&self.db).await.unwrap_or(None)"));
}

#[test]
fn upload_admission_resolves_runtime_settings_instead_of_startup_snapshots() {
    let compact = compact(&source("src/application/upload.rs"));
    assert!(compact.contains("settings:Arc<dynFileSettings>"));
    assert!(!compact.contains("local_root:PathBuf") && !compact.contains("max_editable_size:u64"));
    assert!(compact.contains("self.settings.max_editable_size().await?"));
    assert!(compact.contains("self.settings.local_root().await?"));
    assert!(compact.contains("inline_threshold:max_editable_size"));
    assert!(compact.contains("max_upload_bytes:self.max_upload_size"));
}

#[test]
fn legacy_file_helper_services_are_removed() {
    let services = source("src/services/mod.rs");
    for obsolete in [
        "editor_service", "preview_service", "operation_service", "pub mod files;",
        "EditorService", "PreviewService", "OperationService",
    ] {
        assert!(!services.contains(obsolete), "legacy file helper surface must stay removed: {obsolete}");
    }
    for path in [
        "src/services/editor_service.rs",
        "src/services/preview_service.rs",
        "src/services/operation_service.rs",
        "src/services/files/mod.rs",
    ] {
        assert!(!std::path::Path::new(path).exists(), "legacy file helper must stay deleted: {path}");
    }
}

#[test]
fn transfer_service_is_not_a_root_state_command_facade() {
    let service = compact(&source("src/services/transfer_service.rs"));
    let api = compact(&source("src/api/transfers.rs"));
    assert!(!service.contains("usecrate::state::AppState"));
    assert!(!service.contains("state:&AppState"));
    for obsolete in [
        "pubasyncfncreate_transfer(", "pubasyncfnlist_transfers(", "pubasyncfncancel_transfer(",
        "pubasyncfnretry_transfer(", "pubasyncfndismiss_transfer(", "pubasyncfnclear_finished_transfers(",
    ] {
        assert!(!service.contains(obsolete));
    }
    assert!(api.contains("State(state):State<TransferState>"));
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
    assert!(!settings.contains("SystemSettingsSource"));
    assert!(!state.contains("SystemSettingsSource"));
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

#[test]
fn recursive_chmod_is_local_only() {
    let service = source("src/services/file_api_service.rs");
    let compact = compact(&service);
    assert!(compact.contains("connection.as_str()!=ConnectionId::LOCAL"));
    assert!(service.contains("Recursive CHMOD is only supported for local storage"));
    assert!(compact.contains("SafePath::resolve("));
    assert!(compact.contains("self.authorization.authorize(actor,connection,FileAction::Write)"));
    assert!(compact.contains("self.file_settings.local_root().await?"));
    assert!(compact.contains("self.file_settings.allow_symlinks_outside_root().await?"));
}

#[test]
fn permission_inheritance_failures_are_not_best_effort() {
    let mutation = source("src/application/files/mutation.rs");
    let write = source("src/application/files/write.rs");
    let policy = source("src/domain/policy.rs");
    assert!(!mutation.contains("let _ = provider.set_permissions"));
    assert!(!write.contains("let _ = provider.set_permissions"));
    assert!(mutation.contains("resolve_destination_permissions_strict"));
    assert!(write.contains("resolve_destination_permissions_strict"));
    assert!(mutation.contains("Filesystem mutation committed; recovery required"));
    assert!(write.contains("Filesystem mutation committed; recovery required"));
    assert!(policy.contains("resolve_destination_permissions_strict"));
}
