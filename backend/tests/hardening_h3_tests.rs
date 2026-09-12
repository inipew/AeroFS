use std::fs;
use std::path::PathBuf;

fn source(path: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(root.join(path)).expect("source file should be readable")
}

#[test]
fn app_state_exposes_realtime_substate() {
    let state = source("src/state.rs");
    assert!(state.contains("pub struct RealtimeState"));
    assert!(state.contains("FromRef<AppState> for RealtimeState"));
    assert!(state.contains("pub(crate) realtime: RealtimeState"));
}

#[test]
fn websocket_http_uses_only_realtime_capability() {
    let api = source("src/api/ws.rs");
    assert!(api.contains("State<RealtimeState>"));

    for forbidden in [
        "State<AppState>",
        "AppState",
        "DbPool",
        "EventJournal",
        "sqlx::",
        "state.db",
        "state.event_journal",
        "state.runtime",
        "shutdown_token.clone()",
        "SELECT connection_id FROM permissions",
    ] {
        assert!(
            !api.contains(forbidden),
            "websocket HTTP adapter leaked dependency: {forbidden}"
        );
    }
}

#[test]
fn realtime_service_owns_ws_runtime_dependencies() {
    let service = source("src/services/realtime_service.rs");
    for expected in [
        "db: DbPool",
        "journal: Arc<EventJournal>",
        "shutdown_token: CancellationToken",
        "pub async fn authorized_connections",
        "pub fn is_event_authorized",
        "pub async fn replay",
    ] {
        assert!(
            service.contains(expected),
            "realtime capability missing dependency or operation: {expected}"
        );
    }
    assert!(!service.contains("AppState"));
}

#[test]
fn bootstrap_composes_realtime_once() {
    let bootstrap = source("src/bootstrap.rs");
    assert!(bootstrap.contains("RealtimeState::new(RealtimeService::new("));
    assert!(bootstrap.contains("realtime,"));
}
