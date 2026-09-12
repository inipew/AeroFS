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

    assert!(
        !src.contains("AppState"),
        "ConnectionService must not depend on AppState"
    );
    assert!(
        !src.contains("AuthenticatedUser"),
        "ConnectionService must consume application Actor identity"
    );
    assert!(
        compact.contains("pubstructConnectionService"),
        "connection lifecycle must remain an injected service object"
    );
    assert!(
        compact.contains("actor:&Actor"),
        "connection lifecycle authorization must use Actor"
    );
}

#[test]
fn connection_http_uses_narrow_capability_state() {
    let src = source("src/api/connections.rs");
    let compact = compact(&src);

    assert!(compact.contains("pubstructConnectionState"));
    assert!(compact.contains("State(state):State<ConnectionState>"));
    assert!(
        !compact.contains("State(state):State<AppState>"),
        "connection HTTP handlers must not extract AppState"
    );
}

#[test]
fn bootstrap_loads_providers_through_connection_service() {
    let src = source("src/bootstrap.rs");
    let compact = compact(&src);

    assert!(compact.contains("letconnections=ConnectionService::new("));
    assert!(compact.contains("connections.load_all_providers_from_db().await;"));
    assert!(
        !compact.contains("ConnectionService::load_all_providers_from_db(&state)"),
        "bootstrap must not route provider loading through AppState"
    );
}

#[test]
fn cli_connection_actions_use_lifecycle_service() {
    let src = source("src/cli/commands/connection.rs");
    let compact = compact(&src);

    assert!(compact.contains("letservice=ConnectionService::new("));
    assert!(compact.contains("service.update_connection("));
    assert!(
        !compact.contains("UPDATEconnectionsSETenabled"),
        "CLI enable/disable must not bypass provider lifecycle with direct SQL"
    );
    assert!(
        !compact.contains("ConnectionService::list_connections(&state"),
        "static AppState connection facade must not return"
    );
}
