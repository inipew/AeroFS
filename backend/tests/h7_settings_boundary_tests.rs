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
        !compact.contains("usecrate::state::")
            && !compact.contains("&AppState")
            && !compact.contains(":AppState"),
        "SettingsService must not depend on AppState as a type or module dependency"
    );
    assert!(
        !src.contains("AuthenticatedUser"),
        "SettingsService must consume application Actor identity"
    );
    assert!(src.contains("pub struct SettingsService"));
    assert!(compact.contains("actor:&Actor"));
}

#[test]
fn settings_service_depends_on_ports_not_infrastructure() {
    let src = source("src/services/settings_service.rs");
    let compact = compact(&src);

    for forbidden in [
        "DbPool",
        "ProviderRegistry",
        "ProviderFactory",
        "TransferManager",
        "sqlx::",
        "record_audit_log",
        "crate::infrastructure",
    ] {
        assert!(
            !src.contains(forbidden),
            "SettingsService must not depend on concrete infrastructure: {forbidden}"
        );
    }

    assert!(compact.contains("Arc<dynSystemSettingsStore>"));
    assert!(compact.contains("Arc<dynSettingsRuntime>"));
    assert!(compact.contains("Arc<dynSettingsAudit>"));
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
    assert!(compact.contains("impl_from_ref!(SettingsState,settings)"));
}

#[test]
fn bootstrap_owns_settings_composition() {
    let src = source("src/bootstrap.rs");
    let compact = compact(&src);

    assert!(compact.contains("letsettings_service=SettingsService::new("));
    assert!(compact.contains("SqliteSystemSettingsStore::new(db.clone())"));
    assert!(compact.contains("RegistrySettingsRuntime::new("));
    assert!(compact.contains("SqliteSettingsAudit::new(db.clone())"));
    assert!(compact.contains("letsettings=SettingsState::new(settings_service.clone())"));
    assert!(compact.contains("settings,"));
}

#[test]
fn settings_persistence_remains_transactional() {
    let src = source("src/infrastructure/settings.rs");
    let compact = compact(&src);

    assert!(compact.contains("letmuttx=self.db.begin().await"));
    assert!(compact.contains("tx.commit().await"));
    assert!(compact.contains("ONCONFLICT(key)DOUPDATESET"));
}

#[test]
fn settings_update_prepares_before_persisting_and_activates_after_commit() {
    let src = compact(&source("src/services/settings_service.rs"));
    let update = src
        .split("pubasyncfnupdate_settings(")
        .nth(1)
        .expect("SettingsService must expose update_settings");

    let prepare = update
        .find("prepare_local_root(root).await?")
        .expect("settings update must prepare the local provider before persistence");
    let persist = update
        .find("self.store.upsert_many(&values).await?")
        .expect("settings update must persist settings atomically");
    let activate = update
        .find("prepared.activate().await")
        .expect("prepared provider must be activated after persistence");

    assert!(
        prepare < persist && persist < activate,
        "settings update ordering must remain prepare -> durable commit -> runtime activation"
    );
    assert!(update.contains(
        "set_max_concurrent_transfers(app_settings.transfers.max_concurrent_transfers)"
    ));
}
