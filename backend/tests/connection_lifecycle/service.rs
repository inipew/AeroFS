use backend::{
    domain::{Actor, Connection, ConnectionStatus, ProviderKind},
    ports::connections::{ConnectionRepository, ConnectionRuntime},
    services::connection_service::{
        ConnectionService, CreateConnectionRequest, UpdateConnectionRequest,
    },
};
use chrono::Utc;
use std::sync::Arc;

use crate::support::{
    connection_event_log, RecordingConnectionEffects, RecordingConnectionRepository,
    RecordingConnectionRuntime, SecretFailure, StaticConnectionSettings,
};

fn admin() -> Actor {
    Actor {
        id: "admin-user".into(),
        username: "admin".into(),
        is_admin: true,
    }
}

fn regular() -> Actor {
    Actor {
        id: "regular-user".into(),
        username: "regular".into(),
        is_admin: false,
    }
}

fn connection(id: &str, enabled: bool) -> Connection {
    let now = Utc::now();
    Connection {
        id: id.into(),
        name: format!("Connection {id}"),
        provider: ProviderKind::S3,
        host: Some("example.invalid".into()),
        port: Some(443),
        username: Some("user".into()),
        base_path: "/".into(),
        read_only: false,
        enabled,
        status: if enabled {
            ConnectionStatus::Connected
        } else {
            ConnectionStatus::Disconnected
        },
        error_message: None,
        created_at: now,
        updated_at: now,
    }
}

struct Fixture {
    service: ConnectionService,
    repository: Arc<RecordingConnectionRepository>,
    runtime: Arc<RecordingConnectionRuntime>,
    effects: Arc<RecordingConnectionEffects>,
    events: crate::support::ConnectionEventLog,
}

fn fixture() -> Fixture {
    let events = connection_event_log();
    let repository = Arc::new(RecordingConnectionRepository::new(events.clone()));
    let runtime = Arc::new(RecordingConnectionRuntime::new(events.clone()));
    let effects = Arc::new(RecordingConnectionEffects::new(events.clone()));
    let settings = Arc::new(StaticConnectionSettings::new(std::env::temp_dir()));
    let service = ConnectionService::new(
        repository.clone(),
        settings,
        runtime.clone(),
        effects.clone(),
    );
    Fixture {
        service,
        repository,
        runtime,
        effects,
        events,
    }
}

#[tokio::test]
async fn create_prepares_before_durable_commit_and_activates_after_commit() {
    let fixture = fixture();
    let id = fixture
        .service
        .create_connection(
            &admin(),
            CreateConnectionRequest {
                name: "Primary S3".into(),
                provider: ProviderKind::S3,
                host: Some("storage.example.com".into()),
                port: Some(443),
                username: Some("access".into()),
                secret: Some("top-secret".into()),
                base_path: Some("/bucket".into()),
                read_only: Some(false),
            },
        )
        .await
        .unwrap();

    assert_eq!(fixture.repository.secret(&id).as_deref(), Some("top-secret"));
    assert_eq!(
        fixture.runtime.prepared_secrets(),
        vec![(id.clone(), Some("top-secret".into()))]
    );
    assert_eq!(
        fixture.events.lock().unwrap().clone(),
        vec![
            "runtime.validate_target".to_string(),
            format!("runtime.prepare:{id}"),
            format!("repository.create:{id}"),
            format!("runtime.activate:{id}"),
        ]
    );
}

#[tokio::test]
async fn update_secret_semantics_keep_replace_and_clear_without_losing_ordering() {
    let fixture = fixture();
    let id = "remote-a";
    fixture.repository.insert_connection(connection(id, true));
    fixture.repository.set_secret(id, "original");
    fixture.runtime.set_active(id, true);

    fixture
        .service
        .update_connection(
            &admin(),
            id,
            UpdateConnectionRequest {
                name: Some("Renamed".into()),
                host: None,
                port: None,
                username: None,
                secret: None,
                base_path: None,
                read_only: None,
                enabled: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(fixture.repository.secret(id).as_deref(), Some("original"));
    assert_eq!(
        fixture.runtime.prepared_secrets().last().unwrap().1.as_deref(),
        Some("original")
    );

    fixture
        .service
        .update_connection(
            &admin(),
            id,
            UpdateConnectionRequest {
                name: None,
                host: None,
                port: None,
                username: None,
                secret: Some("replacement".into()),
                base_path: None,
                read_only: None,
                enabled: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(fixture.repository.secret(id).as_deref(), Some("replacement"));
    assert_eq!(
        fixture.runtime.prepared_secrets().last().unwrap().1.as_deref(),
        Some("replacement")
    );

    fixture
        .service
        .update_connection(
            &admin(),
            id,
            UpdateConnectionRequest {
                name: None,
                host: None,
                port: None,
                username: None,
                secret: Some("   ".into()),
                base_path: None,
                read_only: None,
                enabled: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(fixture.repository.secret(id), None);
    assert_eq!(fixture.runtime.prepared_secrets().last().unwrap().1, None);

    let events = fixture.events.lock().unwrap().clone();
    let update_events: Vec<String> = events
        .into_iter()
        .filter(|event| event.contains(id) || event == "runtime.validate_target")
        .collect();
    let one_update = vec![
        "runtime.validate_target".to_string(),
        format!("runtime.prepare:{id}"),
        format!("repository.update:{id}"),
        format!("runtime.activate:{id}"),
        format!("effects.invalidate_metadata:{id}"),
    ];
    let expected: Vec<String> = one_update
        .iter()
        .cloned()
        .chain(one_update.iter().cloned())
        .chain(one_update.iter().cloned())
        .collect();
    assert_eq!(update_events, expected);
}

#[tokio::test]
async fn disabling_connection_persists_then_removes_runtime_and_invalidates_metadata() {
    let fixture = fixture();
    let id = "remote-disabled";
    fixture.repository.insert_connection(connection(id, true));
    fixture.runtime.set_active(id, true);

    fixture
        .service
        .update_connection(
            &admin(),
            id,
            UpdateConnectionRequest {
                name: None,
                host: None,
                port: None,
                username: None,
                secret: None,
                base_path: None,
                read_only: None,
                enabled: Some(false),
            },
        )
        .await
        .unwrap();

    assert_eq!(
        fixture.events.lock().unwrap().clone(),
        vec![
            "runtime.validate_target".to_string(),
            format!("repository.update:{id}"),
            format!("runtime.remove:{id}"),
            format!("effects.invalidate_metadata:{id}"),
        ]
    );
    assert!(!fixture.repository.get(id).await.unwrap().unwrap().enabled);
}

#[tokio::test]
async fn delete_detaches_then_cancels_then_crosses_durable_barrier_before_cache_invalidation() {
    let fixture = fixture();
    let id = "remote-delete";
    fixture.repository.insert_connection(connection(id, true));
    fixture.repository.set_secret(id, "secret");
    fixture.runtime.set_active(id, true);

    fixture.service.delete_connection(&admin(), id).await.unwrap();

    assert_eq!(
        fixture.events.lock().unwrap().clone(),
        vec![
            format!("runtime.detach:{id}"),
            format!("effects.cancel_active_transfers:{id}"),
            format!("repository.delete_with_transfer_barrier:{id}"),
            format!("effects.invalidate_metadata:{id}"),
        ]
    );
    assert!(fixture.repository.get(id).await.unwrap().is_none());
    assert!(fixture.repository.secret(id).is_none());
}

#[tokio::test]
async fn failed_durable_delete_restores_the_exact_detached_runtime() {
    let fixture = fixture();
    let id = "remote-rollback";
    fixture.repository.insert_connection(connection(id, true));
    fixture.runtime.set_active(id, true);
    fixture.repository.fail_delete_once("database unavailable");

    let error = fixture
        .service
        .delete_connection(&admin(), id)
        .await
        .expect_err("durable delete failure must abort lifecycle teardown");
    assert!(error.to_string().contains("database unavailable"));
    assert!(fixture.runtime.info(id).await.active);
    assert_eq!(
        fixture.events.lock().unwrap().clone(),
        vec![
            format!("runtime.detach:{id}"),
            format!("effects.cancel_active_transfers:{id}"),
            format!("repository.delete_with_transfer_barrier:{id}"),
            format!("runtime.restore:{id}"),
        ]
    );
}

#[tokio::test]
async fn cancellation_failure_restores_runtime_before_any_durable_delete() {
    let fixture = fixture();
    let id = "remote-cancel-fail";
    fixture.repository.insert_connection(connection(id, true));
    fixture.runtime.set_active(id, true);
    fixture.effects.fail_cancel_once("cancel failed");

    fixture
        .service
        .delete_connection(&admin(), id)
        .await
        .expect_err("failed cancellation must roll back detached runtime");

    assert!(fixture.runtime.info(id).await.active);
    assert_eq!(
        fixture.events.lock().unwrap().clone(),
        vec![
            format!("runtime.detach:{id}"),
            format!("effects.cancel_active_transfers:{id}"),
            format!("runtime.restore:{id}"),
        ]
    );
    assert!(fixture.repository.get(id).await.unwrap().is_some());
}

#[tokio::test]
async fn startup_isolates_credential_decryption_and_provider_prepare_failures() {
    let fixture = fixture();
    let decrypt_id = "remote-decrypt";
    let prepare_id = "remote-prepare";
    fixture.repository.insert_connection(connection(decrypt_id, true));
    fixture.repository.insert_connection(connection(prepare_id, true));
    fixture.repository.set_secret(prepare_id, "valid-secret");
    fixture.repository.fail_secret(
        decrypt_id,
        SecretFailure::Decryption("invalid ciphertext".into()),
    );
    fixture.runtime.fail_prepare(prepare_id, "provider config rejected");

    fixture.service.load_all_providers_from_db().await.unwrap();

    let decrypt_info = fixture.runtime.info(decrypt_id).await;
    assert!(!decrypt_info.active);
    assert!(decrypt_info
        .error_message
        .as_deref()
        .unwrap()
        .contains("decrypt persisted credential"));
    let prepare_info = fixture.runtime.info(prepare_id).await;
    assert!(!prepare_info.active);
    assert!(prepare_info
        .error_message
        .as_deref()
        .unwrap()
        .contains("provider config rejected"));
    assert!(fixture.runtime.info("local").await.active);
}

#[tokio::test]
async fn startup_treats_credential_persistence_failure_as_fatal() {
    let fixture = fixture();
    let id = "remote-secret-db";
    fixture.repository.insert_connection(connection(id, true));
    fixture
        .repository
        .fail_secret(id, SecretFailure::Persistence("sqlite read failed".into()));

    let error = fixture
        .service
        .load_all_providers_from_db()
        .await
        .expect_err("durable credential read failure must stop startup");
    assert!(error.to_string().contains("sqlite read failed"));
}

#[tokio::test]
async fn non_admin_cannot_mutate_connections_but_explicit_read_permission_is_honored() {
    let fixture = fixture();
    let id = "shared-remote";
    fixture.repository.insert_connection(connection(id, true));
    fixture.repository.allow_read(&regular().id, id);
    fixture.runtime.set_active(id, true);

    assert!(fixture
        .service
        .get_connection(&regular(), id)
        .await
        .is_ok());
    let error = fixture
        .service
        .create_connection(
            &regular(),
            CreateConnectionRequest {
                name: "forbidden".into(),
                provider: ProviderKind::S3,
                host: None,
                port: None,
                username: None,
                secret: None,
                base_path: None,
                read_only: None,
            },
        )
        .await
        .expect_err("regular users cannot create storage connections");
    assert!(error.to_string().contains("Only administrators"));
}
