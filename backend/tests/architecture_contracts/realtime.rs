use crate::support::architecture_source as source;

#[test]
fn websocket_http_uses_only_realtime_capability() {
    let state = source("src/state.rs");
    assert!(state.contains("pub struct RealtimeState"));
    assert!(state.contains("impl_from_ref!(RealtimeState, realtime)"));
    assert!(state.contains("pub(crate) realtime: RealtimeState"));

    let api = source("src/api/ws.rs");
    assert!(api.contains("State<RealtimeState>"));
    assert!(api.contains("ws authorization snapshot failed"));
    assert!(api.contains("ws authorization refresh failed"));
    for forbidden in [
        "State<AppState>",
        "AppState",
        "DbPool",
        "EventJournal",
        "sqlx::",
        "state.db",
        "state.event_journal",
        "state.runtime",
        "SELECT connection_id FROM permissions",
    ] {
        assert!(!api.contains(forbidden), "websocket HTTP adapter leaked {forbidden}");
    }
}

#[test]
fn realtime_service_uses_authorization_port_and_propagates_failures() {
    let service = source("src/services/realtime_service.rs");
    for expected in [
        "authorization: Arc<dyn RealtimeAuthorization>",
        "journal: Arc<EventJournal>",
        "shutdown_token: CancellationToken",
        "pub async fn authorized_connections",
        "-> Result<HashSet<String>, AppError>",
        ".readable_connections(&principal.user_id)",
        "pub fn is_event_authorized",
        "pub async fn replay",
    ] {
        assert!(service.contains(expected), "realtime capability missing {expected}");
    }
    for forbidden in ["AppState", "DbPool", "sqlx::", "unwrap_or_default()"] {
        assert!(!service.contains(forbidden), "realtime service leaked {forbidden}");
    }
}

#[test]
fn bootstrap_composes_realtime_capability_once() {
    let bootstrap = source("src/bootstrap.rs");
    assert_eq!(bootstrap.matches("RealtimeState::new(RealtimeService::new(").count(), 1);
    assert!(bootstrap.contains("realtime: RealtimeState::new("));
    assert!(bootstrap.contains("SqliteRealtimeAuthorization::new(db.clone())"));
}
