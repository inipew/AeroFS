use backend::config::AppConfig;
use backend::db::init_db;
use backend::events::{DomainEvent, EventJournal};
use backend::AppState;
use chrono::Utc;
use sqlx::Row;
use std::time::Duration;
use tempfile::tempdir;

#[tokio::test]
async fn durable_consumer_cursor_protects_unprocessed_journal_rows_from_vacuum() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("phase8_journal.db");
    let database_url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    let db = init_db(&database_url).await.unwrap();
    let journal = EventJournal::init(db.clone()).await.unwrap();

    let first = journal
        .append(DomainEvent::file_change("local", "/one.txt", "write"), None)
        .await
        .unwrap();
    let second = journal
        .append(DomainEvent::file_change("local", "/two.txt", "write"), None)
        .await
        .unwrap();
    let first_id = first.journal_id.expect("first event must be durable");
    let second_id = second.journal_id.expect("second event must be durable");

    // Make both rows old enough to be vacuum candidates independent of wall clock.
    sqlx::query("UPDATE event_journal SET created_at = '2000-01-01T00:00:00Z'")
        .execute(&db)
        .await
        .unwrap();

    // Loading a cursor also registers the consumer at zero. No unprocessed row may
    // be removed while this projection is known but has not advanced yet.
    assert_eq!(journal.consumer_cursor("phase8-test").await.unwrap(), 0);
    assert_eq!(journal.vacuum(Duration::ZERO).await.unwrap(), 0);

    journal
        .store_consumer_cursor("phase8-test", first_id)
        .await
        .unwrap();
    assert_eq!(journal.vacuum(Duration::ZERO).await.unwrap(), 1);

    let remaining: Vec<i64> = sqlx::query_scalar("SELECT id FROM event_journal ORDER BY id")
        .fetch_all(&db)
        .await
        .unwrap();
    assert_eq!(remaining, vec![second_id]);
}

#[tokio::test]
async fn sync_transfer_completion_projection_is_idempotent() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("phase8_sync.db");
    let storage_dir = temp.path().join("storage");
    std::fs::create_dir_all(&storage_dir).unwrap();

    let mut config = AppConfig::default();
    config.database.url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    config.filesystem.default_local_root = storage_dir;

    let db = init_db(&config.database.url).await.unwrap();
    let state = AppState::new_with_db(config, db.clone()).await;
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO sync_jobs (id, user_id, source_connection_id, source_path, destination_connection_id, destination_path, status, strategy, total_files, synced_files, conflict_files, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("sync-phase8")
    .bind("user-phase8")
    .bind("local")
    .bind("/source")
    .bind("local")
    .bind("/destination")
    .bind("executing")
    .bind("keep_both")
    .bind(1_i64)
    .bind(0_i64)
    .bind(0_i64)
    .bind(&now)
    .bind(&now)
    .execute(&db)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO sync_operations (id, job_id, op_kind, relative_path, old_path, status, transfer_job_id, error_message, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("op-phase8")
    .bind("sync-phase8")
    .bind("create")
    .bind("file.txt")
    .bind(None::<String>)
    .bind("running")
    .bind("transfer-phase8")
    .bind(None::<String>)
    .bind(&now)
    .bind(&now)
    .execute(&db)
    .await
    .unwrap();

    // Simulate at-least-once delivery: the same durable completion arrives twice.
    state
        .sync_manager
        .notify_transfer_completed("transfer-phase8", true)
        .await
        .unwrap();
    state
        .sync_manager
        .notify_transfer_completed("transfer-phase8", true)
        .await
        .unwrap();

    let row = sqlx::query("SELECT synced_files, status FROM sync_jobs WHERE id = ?")
        .bind("sync-phase8")
        .fetch_one(&db)
        .await
        .unwrap();
    let synced_files: i64 = row.get("synced_files");
    let job_status: String = row.get("status");
    assert_eq!(synced_files, 1, "completion replay must not double count");
    assert_eq!(job_status, "completed");

    let op_status: String =
        sqlx::query_scalar("SELECT status FROM sync_operations WHERE id = ?")
            .bind("op-phase8")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(op_status, "completed");
}
