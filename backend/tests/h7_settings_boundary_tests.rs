use std::fs;

fn source(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"))
}

fn compact(src: &str) -> String {
    src.chars().filter(|c| !c.is_whitespace()).collect()
}

#[test]
fn settings_service_is_not_an_app_state_facade() {
    let src = source("src/services/settings_service.rs");
    let compact = compact(&src);

    assert!(
        !src.contains("AppState"),
        "SettingsService must not depend on AppState"
    );
    assert!(
        !src.contains("AuthenticatedUser"),
        "SettingsService must consume application Actor identity"
    );
    assert!(src.contains("pub struct SettingsService"));
    assert!(compact.contains("actor:&Actor"));
}

#[test]
fn settings_http_uses_narrow_capability_state() {
    let src = source("src/api/settings.rs");
    let compact = compact(&src);

    assert!(compact.contains("State(state):State<SettingsState>"));
    assert!(
        !compact.contains("State(state):State<AppState>"),
        "settings HTTP handlers must not extract AppState"
    );
    assert!(compact.contains("state.service.get_settings(&actor(&user))"));
    assert!(compact.contains("state.service.update_settings(&actor(&user),payload)"));
}

#[test]
fn app_state_stores_precomposed_settings_capability() {
    let src = source("src/state.rs");
    let compact = compact(&src);

    assert!(src.contains("pub struct SettingsState"));
    assert!(compact.contains("pub(crate)settings:SettingsState"));
    assert!(compact.contains("FromRef<AppState>forSettingsState"));
    assert!(compact.contains("state.settings.clone()"));
}

#[test]
fn bootstrap_owns_settings_composition() {
    let src = source("src/bootstrap.rs");
    let compact = compact(&src);

    assert!(compact.contains("SettingsState::new(SettingsService::new("));
    assert!(compact.contains("settings,"));
}

#[test]
fn settings_update_preserves_transaction_and_runtime_effects() {
    let src = source("src/services/settings_service.rs");
    let compact = compact(&src);

    assert!(compact.contains("self.db.begin().await"));
    assert!(compact.contains("tx.commit().await"));
    assert!(compact.contains("self.refresh_local_root_runtime(root_path).await"));
    assert!(compact.contains("set_max_concurrent_transfers(app_settings.transfers.max_concurrent_transfers)"));
    assert!(src.contains("SETTINGS_UPDATED"));
}
