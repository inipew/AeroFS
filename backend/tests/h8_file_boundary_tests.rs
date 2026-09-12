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
}

#[test]
fn permission_inheritance_failures_are_not_best_effort() {
    let mutation = source("src/application/files/mutation.rs");
    let write = source("src/application/files/write.rs");
    let policy = source("src/domain/policy.rs");

    assert!(
        !mutation.contains("let _ = provider.set_permissions"),
        "directory creation must not swallow inherited permission failures"
    );
    assert!(
        !write.contains("let _ = provider.set_permissions"),
        "file writes must not swallow inherited permission failures"
    );
    assert!(
        mutation.contains("Filesystem mutation committed; recovery required"),
        "post-create permission failure must expose partial-commit recovery semantics"
    );
    assert!(
        write.contains("Filesystem mutation committed; recovery required"),
        "direct-write permission failure must expose partial-commit recovery semantics"
    );
    assert!(
        policy.contains("Result<Option<String>, VfsError>"),
        "permission resolution must distinguish provider failure from no permission value"
    );
}
