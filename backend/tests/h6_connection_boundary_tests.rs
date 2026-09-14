use std::fs;

fn source(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"))
}

fn compact(src: &str) -> String {
    src.chars().filter(|ch| !ch.is_whitespace()).collect()
}

#[test]
fn connection_service_depends_only_on_application_ports() {
    let src = source("src/services/connection_service.rs");
    let compact = compact(&src);
    for forbidden in [
        "AppState",
        "AuthenticatedUser",
        "DbPool",
        "CredentialStore",
        "ProviderFactory",
        "ProviderRegistry",
        "TransferManager",
        "MetadataCache",
        "AppConfig",
        "sqlx::",
        "crate::security",
    ] {
        assert!(
            !src.contains(forbidden),
            "ConnectionService must not depend on concrete runtime/persistence detail `{forbidden}`"
        );
    }
    assert!(compact.contains("repository:Arc<dynConnectionRepository>"));
    assert!(compact.contains("settings:Arc<dynFileSettings>"));
    assert!(compact.contains("runtime:Arc<dynConnectionRuntime>"));
    assert!(compact.contains("effects:Arc<dynConnectionEffects>"));
    assert!(compact.contains("actor:&Actor"));
}

#[test]
fn connection_http_uses_precomposed_narrow_capability_state() {
    let api = source("src/api/connections.rs");
    let state = source("src/state.rs");
    let compact_api = compact(&api);
    let compact_state = compact(&state);

    assert!(compact_api.contains("State(state):State<ConnectionState>"));
    assert!(!compact_api.contains("State(state):State<AppState>"));
    assert!(!api.contains("ConnectionService::new"));
    assert!(compact_state.contains("pubstructConnectionState"));
    assert!(compact_state.contains("impl_from_ref!(ConnectionState,connections)"));
}

#[test]
fn bootstrap_owns_connection_repository_runtime_and_effects_composition() {
    let src = source("src/bootstrap.rs");
    let compact = compact(&src);
    assert!(compact.contains("SqliteConnectionRepository::new(db.clone(),credentials)"));
    assert!(compact.contains("RegistryConnectionRuntime::new("));
    assert!(compact.contains("config.clone(),registry.clone(),"));
    assert!(compact.contains("RuntimeConnectionEffects::new("));
    assert!(compact.contains("transfer_manager.clone(),metadata_cache.clone(),"));
    assert!(compact.contains("letconnection_service=ConnectionService::new("));
    assert!(compact.contains(
        "connection_service.load_all_providers_from_db().await.expect(\"Failedtoloadpersistedstorageconnectionstate\")"
    ));
    assert!(compact.contains("connections:ConnectionState::new(connection_service)"));
}

#[test]
fn runtime_and_effects_adapters_own_concrete_lifecycle_dependencies() {
    let runtime = source("src/infrastructure/connection_runtime.rs");
    let compact_runtime = compact(&runtime);

    assert!(runtime.contains("ProviderFactory"));
    assert!(runtime.contains("ProviderRegistry"));
    assert!(runtime.contains("TransferManager"));
    assert!(runtime.contains("MetadataCache"));
    assert!(compact_runtime.contains("implConnectionRuntimeforRegistryConnectionRuntime"));
    assert!(compact_runtime.contains("implConnectionEffectsforRuntimeConnectionEffects"));
    assert!(compact_runtime.contains("validate_after_dns("));
    assert!(compact_runtime.contains("self.registry.register_runtime(self.id.clone(),self.runtime).await"));
}

#[test]
fn cli_connection_actions_use_precomposed_lifecycle_capability() {
    let src = source("src/cli/commands/connection.rs");
    let compact = compact(&src);

    assert!(compact.contains("ConnectionState::from_ref(&state)"));
    assert!(compact.contains(".service"));
    assert!(compact.contains("service.update_connection("));
    assert!(!compact.contains("ConnectionService::new("));
    assert!(!compact.contains("UPDATEconnectionsSETenabled"));

    for forbidden in [
        "state.db",
        "state.config",
        "state.registry",
        "state.credentials",
        "state.metadata_cache",
        "state.transfer_manager",
    ] {
        assert!(!src.contains(forbidden), "CLI leaked raw state dependency {forbidden}");
    }
}
