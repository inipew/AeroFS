use backend::{
    domain::{Connection, ConnectionStatus, ProviderKind},
    infrastructure::{connections::SqliteConnectionRepository, CredentialStore},
    ports::connections::{ConnectionRepository, SecretMutation},
};
use chrono::Utc;
use std::sync::Arc;

use crate::support::TestDatabase;

fn connection(id: &str) -> Connection {
    let now = Utc::now();
    Connection {
        id: id.into(),
        name: format!("Storage {id}"),
        provider: ProviderKind::S3,
        host: Some("bucket.example.com".into()),
        port: Some(443),
        username: Some("access-key".into()),
        base_path: "/prefix".into(),
        read_only: false,
        enabled: true,
        status: ConnectionStatus::Connected,
        error_message: None,
        created_at: now,
        updated_at: now,
    }
}

async fn repository(
    filename: &str,
) -> (TestDatabase, SqliteConnectionRepository, Arc<CredentialStore>) {
    let database = TestDatabase::migrated(filename).await;
    let credentials = Arc::new(CredentialStore::new(
        "connection-lifecycle-test-secret-1234567890",
    ));
    let repository = SqliteConnectionRepository::new(database.pool.clone(), credentials.clone());
    (database, repository, credentials)
}

#[tokio::test]
async fn credentials_are_encrypted_at_rest_and_round_trip_through_repository() {
    let (database, repository, _) = repository("connection-secret-roundtrip.db").await;
    let connection = connection("secret-roundtrip");
    repository
        .create(&connection, Some("plaintext-secret"))
        .await
        .unwrap();

    let stored: (String,) = sqlx::query_as(
        "SELECT encrypted_secret FROM connection_credentials WHERE connection_id = ?",
    )
    .bind(&connection.id)
    .fetch_one(&database.pool)
    .await
    .unwrap();
    assert_ne!(stored.0, "plaintext-secret");
    assert!(!stored.0.contains("plaintext-secret"));
    assert_eq!(
        repository.load_secret(&connection.id).await.unwrap().as_deref(),
        Some("plaintext-secret")
    );
}

#[tokio::test]
async fn credential_updates_distinguish_keep_replace_and_clear_transactionally() {
    let (database, repository, _) = repository("connection-secret-mutations.db").await;
    let mut connection = connection("secret-mutations");
    repository
        .create(&connection, Some("original"))
        .await
        .unwrap();

    connection.name = "Keep secret".into();
    repository
        .update(&connection, SecretMutation::Keep)
        .await
        .unwrap();
    assert_eq!(
        repository.load_secret(&connection.id).await.unwrap().as_deref(),
        Some("original")
    );

    connection.name = "Replace secret".into();
    repository
        .update(&connection, SecretMutation::Replace("replacement".into()))
        .await
        .unwrap();
    assert_eq!(
        repository.load_secret(&connection.id).await.unwrap().as_deref(),
        Some("replacement")
    );

    connection.name = "Clear secret".into();
    repository
        .update(&connection, SecretMutation::Clear)
        .await
        .unwrap();
    assert_eq!(repository.load_secret(&connection.id).await.unwrap(), None);

    let row: (String,) = sqlx::query_as("SELECT name FROM connections WHERE id = ?")
        .bind(&connection.id)
        .fetch_one(&database.pool)
        .await
        .unwrap();
    assert_eq!(row.0, "Clear secret");
}

#[tokio::test]
async fn delete_barrier_fences_active_transfers_and_removes_connection_and_secret_atomically() {
    let (database, repository, _) = repository("connection-delete-barrier.db").await;
    let connection = connection("delete-barrier");
    repository
        .create(&connection, Some("delete-me"))
        .await
        .unwrap();
    let now = Utc::now().to_rfc3339();

    for (id, status) in [
        ("queued-job", "queued"),
        ("running-job", "running"),
        ("requested-job", "cancellation_requested"),
        ("completed-job", "completed"),
    ] {
        sqlx::query(
            "INSERT INTO transfer_jobs (id, name, transfer_type, source_connection_id, source_path, destination_connection_id, destination_path, status, created_at, updated_at) VALUES (?, ?, 'copy', ?, '/src', 'local', '/dst', ?, ?, ?)",
        )
        .bind(id)
        .bind(id)
        .bind(&connection.id)
        .bind(status)
        .bind(&now)
        .bind(&now)
        .execute(&database.pool)
        .await
        .unwrap();
    }

    assert!(repository
        .delete_with_transfer_barrier(&connection.id)
        .await
        .unwrap());

    let statuses: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, status FROM transfer_jobs ORDER BY id ASC",
    )
    .fetch_all(&database.pool)
    .await
    .unwrap();
    assert_eq!(
        statuses,
        vec![
            ("completed-job".into(), "completed".into()),
            ("queued-job".into(), "cancelled".into()),
            ("requested-job".into(), "cancellation_requested".into()),
            ("running-job".into(), "cancellation_requested".into()),
        ]
    );

    let connection_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM connections WHERE id = ?")
            .bind(&connection.id)
            .fetch_one(&database.pool)
            .await
            .unwrap();
    let secret_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM connection_credentials WHERE connection_id = ?")
            .bind(&connection.id)
            .fetch_one(&database.pool)
            .await
            .unwrap();
    assert_eq!(connection_count.0, 0);
    assert_eq!(secret_count.0, 0);
}

#[tokio::test]
async fn repository_surfaces_closed_database_failures_instead_of_defaulting() {
    let (database, repository, _) = repository("connection-db-failure.db").await;
    database.pool.close().await;

    assert!(repository.load_enabled().await.is_err());
    assert!(repository.list(None, true).await.is_err());
    assert!(repository.get("missing").await.is_err());
    assert!(repository.load_secret("missing").await.is_err());
}
