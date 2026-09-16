use backend::db::{
    backup_db, check_integrity, checkpoint_db, connect_db, get_db_stats, migrate_db, vacuum_db,
};

use crate::support::TestDatabase;

#[tokio::test]
async fn sqlite_runtime_pragmas_integrity_and_backup_maintenance_work() {
    let database = TestDatabase::seeded("platform_maintenance.db").await;

    let journal_mode: (String,) = sqlx::query_as("PRAGMA journal_mode;")
        .fetch_one(&database.pool)
        .await
        .unwrap();
    assert_eq!(journal_mode.0.to_lowercase(), "wal");

    let foreign_keys: (i64,) = sqlx::query_as("PRAGMA foreign_keys;")
        .fetch_one(&database.pool)
        .await
        .unwrap();
    assert_eq!(foreign_keys.0, 1);

    let reports = check_integrity(&database.pool).await.unwrap();
    assert!(reports.iter().any(|report| report.contains("integrity_check: ok")));
    assert!(reports
        .iter()
        .any(|report| report.contains("foreign_key_check: ok")));
    vacuum_db(&database.pool).await.unwrap();
    checkpoint_db(&database.pool).await.unwrap();

    let backup_path = database.temp.path().join("backups/snapshot.db");
    backup_db(&database.pool, &backup_path, false).await.unwrap();
    assert!(backup_path.exists());
    assert!(backup_db(&database.pool, &backup_path, false).await.is_err());
    backup_db(&database.pool, &backup_path, true).await.unwrap();
}

#[tokio::test]
async fn connect_migrate_and_stats_keep_schema_initialization_explicit_and_urls_sanitized() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("platform_isolation.db");
    let url = format!("sqlite://{}?mode=rwc", path.to_string_lossy());
    let pool = connect_db(&url).await.unwrap();

    let users_table: Option<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='table' AND name='users'",
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(users_table.is_none());

    let applied = migrate_db(&pool).await.unwrap();
    assert!(!applied.is_empty());

    let stats = get_db_stats(&pool, "sqlite://username:secret@127.0.0.1/test.db")
        .await
        .unwrap();
    assert_eq!(stats.journal_mode, "WAL");
    assert!(stats.foreign_keys);
    assert!(stats.sanitized_url.contains("***:***@"));
    assert!(!stats.sanitized_url.contains("secret"));
}
