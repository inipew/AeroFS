use std::{fs, path::PathBuf};

fn source(path: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(root.join(path)).expect("source file should be readable")
}

#[test]
fn settings_service_uses_application_identity_and_ports_only() {
    let service = source("src/services/settings_service.rs");

    for required in [
        "crate::domain::{settings::*, Actor}",
        "SystemSettingsStore",
        "SettingsRuntime",
        "SettingsAudit",
    ] {
        assert!(service.contains(required), "settings service missing boundary: {required}");
    }

    for forbidden in [
        "AppState",
        "AuthenticatedUser",
        "DbPool",
        "ProviderRegistry",
        "TransferManager",
        "sqlx::",
        "crate::infrastructure",
    ] {
        assert!(
            !service.contains(forbidden),
            "settings service leaked concrete dependency: {forbidden}"
        );
    }
}

#[test]
fn settings_and_preferences_http_use_narrow_capability_states() {
    let settings = source("src/api/settings.rs");
    assert!(settings.contains("State<SettingsState>"));
    assert!(!settings.contains("State<AppState>"));
    assert!(!settings.contains("sqlx::"));

    let preferences = source("src/api/preferences.rs");
    assert!(preferences.contains("State<PreferencesState>"));
    assert!(!preferences.contains("State<AppState>"));
    assert!(!preferences.contains("sqlx::"));
}

#[test]
fn bootstrap_owns_settings_and_preferences_composition_once() {
    let bootstrap = source("src/bootstrap.rs");

    assert_eq!(bootstrap.matches("SettingsService::new(").count(), 1);
    assert_eq!(bootstrap.matches("PreferencesService::new(").count(), 1);
    assert!(bootstrap.contains("SqliteSystemSettingsStore::new(db.clone())"));
    assert!(bootstrap.contains("RegistrySettingsRuntime::new("));
    assert!(bootstrap.contains("SqliteSettingsAudit::new(db.clone())"));
    assert!(bootstrap.contains("SqliteUserPreferencesRepository::new(db.clone())"));
}

#[test]
fn app_state_exposes_settings_and_preferences_as_precomposed_capabilities() {
    let state = source("src/state.rs");

    for required in [
        "pub struct SettingsState",
        "pub struct PreferencesState",
        "impl_from_ref!(SettingsState, settings)",
        "impl_from_ref!(PreferencesState, preferences)",
    ] {
        assert!(state.contains(required), "state missing capability boundary: {required}");
    }
}
