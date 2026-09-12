use std::fs;

fn source(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"))
}

fn compact(src: &str) -> String {
    src.chars().filter(|ch| !ch.is_whitespace()).collect()
}

#[test]
fn connection_service_is_not_an_app_state_facade() {
    let src = source("src/services/connection_service.rs");
    let compact = compact(&src);
    assert!(!src.contains("AppState"));
    assert!(!src.contains("AuthenticatedUser"));
    assert!(compact.contains("pubstructConnectionService"));
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
fn bootstrap_precomposes_and_loads_connection_capability() {
    let src = source("src/bootstrap.rs");
    let compact = compact(&src);
    assert!(compact.contains("letconnection_service=ConnectionService::new("));
    assert!(compact.contains("connection_service.load_all_providers_from_db().await;"));
    assert!(compact.contains("connections:ConnectionState::new(connection_service)"));
}

#[test]
fn cli_connection_actions_use_lifecycle_service() {
    let src = source("src/cli/commands/connection.rs");
    let compact = compact(&src);
    assert!(compact.contains("letservice=ConnectionService::new("));
    assert!(compact.contains("service.update_connection("));
    assert!(!compact.contains("UPDATEconnectionsSETenabled"));
    assert!(!compact.contains("ConnectionService::list_connections(&state"));
}
