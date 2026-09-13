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
    assert!(compact.contains(
        "connection_service.load_all_providers_from_db().await.expect(\"Failedtoloadpersistedstorageconnectionstate\")"
    ));
    assert!(compact.contains("connections:ConnectionState::new(connection_service)"));
}

#[test]
fn persisted_connection_startup_failures_are_not_silenced() {
    let src = source("src/services/connection_service.rs");
    let compact = compact(&src);
    let loader = compact
        .split("pubasyncfnload_all_providers_from_db(&self)->Result<(),AppError>{")
        .nth(1)
        .expect("connection startup loader must return Result");
    let loader = loader
        .split("pubasyncfnlist_connections")
        .next()
        .unwrap();

    for forbidden in [
        ".await.ok().flatten()",
        ".await.unwrap_or_default()",
        ".await.unwrap_or(None)",
        ".decrypt(&r.0).ok()",
    ] {
        assert!(
            !loader.contains(forbidden),
            "connection startup must not silently discard persistent-state failure `{forbidden}`"
        );
    }
    assert!(loader.contains("Failedtoloadlocal_rootsetting"));
    assert!(loader.contains("Failedtoloadenabledstorageconnections"));
    assert!(loader.contains("Failedtoloadcredentialforstorageconnection"));
    assert!(loader.contains("self.registry.set_connection_error(&id,&message).await"));
}

#[test]
fn existing_credentials_are_strictly_loaded_during_connection_update() {
    let src = compact(&source("src/services/connection_service.rs"));
    let update = src
        .split("pubasyncfnupdate_connection(")
        .nth(1)
        .expect("update_connection must exist")
        .split("pubasyncfndelete_connection")
        .next()
        .unwrap();

    assert!(!update.contains(".await.unwrap_or(None)"));
    assert!(!update.contains(".decrypt(&encrypted).ok()"));
    assert!(update.contains("Failedtoloadexistingcredential"));
    assert!(update.contains("Failedtodecryptexistingcredentialfor"));
}

#[test]
fn connection_delete_unpublishes_before_cancellation_and_persists_a_barrier_before_delete() {
    let src = compact(&source("src/services/connection_service.rs"));
    let delete = src
        .split("pubasyncfndelete_connection(")
        .nth(1)
        .expect("delete_connection must exist")
        .split("pubasyncfntest_connection")
        .next()
        .unwrap();

    let unpublish = delete
        .find("self.registry.remove(id).await")
        .expect("delete must unpublish provider before lifecycle teardown");
    let cancel = delete
        .find("self.transfer_manager.cancel_job(&job.id,None,true).await")
        .expect("delete must request cancellation for active transfers");
    let barrier = delete
        .find("UPDATEtransfer_jobsSETstatus=CASEWHENstatus='queued'THEN'cancelled'ELSE'cancellation_requested'END")
        .expect("delete must durably fence active transfer state");
    let durable_delete = delete
        .find("DELETEFROMconnectionsWHEREid=?")
        .expect("delete must remove durable connection state");

    assert!(
        unpublish < cancel && cancel < barrier && barrier < durable_delete,
        "connection deletion ordering must remain unpublish -> cancel live jobs -> persist cancellation barrier -> durable delete"
    );
    assert!(delete.contains("self.registry.register_runtime(id.to_string(),runtime).await"));
    assert!(
        !delete.contains("let_=self.transfer_manager.cancel_job("),
        "connection deletion must not silently discard cancellation errors"
    );
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
