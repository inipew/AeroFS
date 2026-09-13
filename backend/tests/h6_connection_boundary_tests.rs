use std::fs;

fn source(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"))
}

fn compact(src: &str) -> String {
    src.chars().filter(|ch| !ch.is_whitespace()).collect()
}

#[test]
fn connection_service_is_not_an_app_state_or_persistence_facade() {
    let src = source("src/services/connection_service.rs");
    let compact = compact(&src);
    for forbidden in [
        "AppState",
        "AuthenticatedUser",
        "DbPool",
        "CredentialStore",
        "sqlx::",
    ] {
        assert!(
            !src.contains(forbidden),
            "ConnectionService must not depend on persistence detail `{forbidden}`"
        );
    }
    assert!(compact.contains("pubstructConnectionService"));
    assert!(compact.contains("repository:Arc<dynConnectionRepository>"));
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
fn bootstrap_precomposes_repository_and_connection_capability() {
    let src = source("src/bootstrap.rs");
    let compact = compact(&src);
    assert!(compact.contains("SqliteConnectionRepository::new(db.clone(),credentials)"));
    assert!(compact.contains("letconnection_service=ConnectionService::new("));
    assert!(compact.contains(
        "connection_service.load_all_providers_from_db().await.expect(\"Failedtoloadpersistedstorageconnectionstate\")"
    ));
    assert!(compact.contains("connections:ConnectionState::new(connection_service)"));
}

#[test]
fn persisted_connection_startup_failures_are_not_silenced() {
    let service = compact(&source("src/services/connection_service.rs"));
    let repository = compact(&source("src/infrastructure/connections.rs"));
    let loader = service
        .split("pubasyncfnload_all_providers_from_db(&self)->Result<(),AppError>{")
        .nth(1)
        .expect("connection startup loader must return Result")
        .split("pubasyncfnlist_connections")
        .next()
        .unwrap();

    assert!(loader.contains("self.repository.local_root_override().await?"));
    assert!(loader.contains("self.repository.load_enabled().await?"));
    assert!(loader.contains("ConnectionSecretError::Persistence(error)"));
    assert!(loader.contains("ConnectionSecretError::Decryption(error)"));
    assert!(loader.contains("self.registry.set_connection_error(&connection.id,&message).await"));

    for forbidden in [
        ".await.ok().flatten()",
        ".await.unwrap_or_default()",
        ".await.unwrap_or(None)",
    ] {
        assert!(
            !repository.contains(forbidden),
            "connection repository must not silently discard persistent-state failure `{forbidden}`"
        );
    }
}

#[test]
fn credential_update_semantics_distinguish_keep_replace_and_clear() {
    let service = compact(&source("src/services/connection_service.rs"));
    let repository = compact(&source("src/infrastructure/connections.rs"));
    let update = service
        .split("pubasyncfnupdate_connection(")
        .nth(1)
        .expect("update_connection must exist")
        .split("pubasyncfndelete_connection")
        .next()
        .unwrap();

    assert!(update.contains("SecretMutation::Clear"));
    assert!(update.contains("SecretMutation::Replace(secret)"));
    assert!(update.contains("SecretMutation::Keep"));
    assert!(update.contains("self.repository.load_secret(id).await"));
    assert!(repository.contains("SecretMutation::Clear=>"));
    assert!(repository.contains("DELETEFROMconnection_credentialsWHEREconnection_id=?"));
}

#[test]
fn connection_delete_unpublishes_and_cancels_before_repository_barrier() {
    let service = compact(&source("src/services/connection_service.rs"));
    let repository = compact(&source("src/infrastructure/connections.rs"));
    let delete = service
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
    let durable_teardown = delete
        .find("self.repository.delete_with_transfer_barrier(id).await?")
        .expect("delete must delegate atomic durable teardown to repository");
    assert!(unpublish < cancel && cancel < durable_teardown);
    assert!(delete.contains("self.registry.register_runtime(id.to_string(),runtime).await"));
    assert!(!delete.contains("let_=self.transfer_manager.cancel_job("));

    let barrier = repository
        .find("UPDATEtransfer_jobsSETstatus=CASEWHENstatus='queued'THEN'cancelled'ELSE'cancellation_requested'END")
        .expect("repository must durably fence active transfer state");
    let durable_delete = repository
        .find("DELETEFROMconnectionsWHEREid=?")
        .expect("repository must remove durable connection state");
    assert!(barrier < durable_delete);
    assert!(repository.contains("tx.commit().await"));
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
