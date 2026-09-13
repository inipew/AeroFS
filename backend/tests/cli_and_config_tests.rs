use backend::bootstrap::build_user_service;
use backend::cli::daemon_lock::{DaemonLock, ProcessStatus};
use backend::config::AppConfig;
use backend::db::{
    backup_db, check_integrity, checkpoint_db, connect_db, get_db_stats, init_db, migrate_db,
    vacuum_db,
};
use backend::infrastructure::transfer_history::SqliteTransferHistoryRepository;
use backend::services::TransferService;
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;

#[tokio::test]
async fn test_config_hierarchy_and_toml_loading() {
    let temp = tempdir().unwrap();
    let config_path = temp.path().join("aerofs_custom.toml");

    let toml_content = r#"
[server]
host = "0.0.0.0"
port = 9090

[filesystem]
default_local_root = "/tmp/aerofs_test_storage"
show_hidden_default = true
read_only_default = false

[limits]
max_upload_size = 524288000
max_editable_size = 5242880
max_preview_size = 10485760
max_directory_entries = 10000
max_concurrent_transfers = 8

[security]
session_secret = "custom_secret_key_that_is_long_enough_for_security_123"
session_ttl_secs = 3600
allow_symlinks_outside_root = true
allow_private_network_connections = false

[database]
url = "sqlite:///tmp/aerofs_test.db?mode=rwc"
"#;

    fs::write(&config_path, toml_content).unwrap();

    let config = AppConfig::load(Some(&config_path)).unwrap();
    assert_eq!(config.server.host, "0.0.0.0");
    assert_eq!(config.server.port, 9090);
    assert_eq!(
        config.filesystem.default_local_root.to_str().unwrap(),
        "/tmp/aerofs_test_storage"
    );
    assert!(config.filesystem.show_hidden_default);
    assert_eq!(config.limits.max_concurrent_transfers, 8);
    assert!(config.security.allow_symlinks_outside_root);
    assert!(!config.security.allow_private_network_connections);

    let sanitized = config.to_sanitized_toml();
    assert!(sanitized.contains("********"));
    assert!(!sanitized.contains("custom_secret_key_that_is_long_enough"));

    let mut invalid_cfg = config.clone();
    invalid_cfg.server.port = 0;
    assert!(invalid_cfg.validate().is_err());
}

#[tokio::test]
async fn test_sqlite_wal_and_foreign_keys_pragmas() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("wal_test.db");
    let db_url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());

    let pool = init_db(&db_url).await.unwrap();
    let row: (String,) = sqlx::query_as("PRAGMA journal_mode;")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.0.to_lowercase(), "wal");

    let row_fk: (i64,) = sqlx::query_as("PRAGMA foreign_keys;")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row_fk.0, 1);

    let reports = check_integrity(&pool).await.unwrap();
    assert!(reports.iter().any(|r| r.contains("integrity_check: ok")));
    assert!(reports.iter().any(|r| r.contains("foreign_key_check: ok")));
    assert!(vacuum_db(&pool).await.is_ok());
    assert!(checkpoint_db(&pool).await.is_ok());

    let backup_path = temp.path().join("backups/snapshot.db");
    assert!(backup_db(&pool, &backup_path, false).await.is_ok());
    assert!(backup_path.exists());
    assert!(backup_db(&pool, &backup_path, false).await.is_err());
    assert!(backup_db(&pool, &backup_path, true).await.is_ok());
}

#[tokio::test]
async fn test_db_isolation_and_stats() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("isolation_test.db");
    let db_url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    let pool = connect_db(&db_url).await.unwrap();

    let table_exists: Option<(String,)> =
        sqlx::query_as("SELECT name FROM sqlite_master WHERE type='table' AND name='users'")
            .fetch_optional(&pool)
            .await
            .unwrap();
    assert!(table_exists.is_none());

    let applied = migrate_db(&pool).await.unwrap();
    assert!(!applied.is_empty(), "migrate_db should apply migrations");

    let stats = get_db_stats(&pool, "sqlite://username:secret@127.0.0.1/test.db")
        .await
        .unwrap();
    assert_eq!(stats.journal_mode, "WAL");
    assert!(stats.foreign_keys);
    assert!(stats.sanitized_url.contains("***:***@"));
    assert!(!stats.sanitized_url.contains("secret"));
}

#[tokio::test]
async fn test_daemon_lock_lifecycle_and_status() {
    let temp = tempdir().unwrap();
    let lock_path = temp.path().join("test_aerofs.lock");
    let status = DaemonLock::inspect_status(&lock_path, "127.0.0.1", 8080);
    assert_eq!(status, ProcessStatus::Stopped);

    let lock1 = DaemonLock::acquire(&lock_path).unwrap();
    assert!(lock_path.exists());
    assert!(DaemonLock::acquire(&lock_path).is_err());

    let running_status = DaemonLock::inspect_status(&lock_path, "127.0.0.1", 8080);
    match running_status {
        ProcessStatus::Running { pid, .. } => assert_eq!(pid, std::process::id()),
        other => panic!("Expected Running status, got: {:?}", other),
    }

    lock1.release();
    assert!(!lock_path.exists());
    assert_eq!(
        DaemonLock::inspect_status(&lock_path, "127.0.0.1", 8080),
        ProcessStatus::Stopped
    );
}

#[tokio::test]
async fn test_user_service_safeguards() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("user_safeguard_test.db");
    let db_url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    let pool = init_db(&db_url).await.unwrap();
    let users = build_user_service(pool);

    let user_list = users.list_users().await.unwrap();
    assert_eq!(user_list.len(), 1);
    assert_eq!(user_list[0].username, "admin");
    assert!(user_list[0].is_admin);

    let del_err = users.delete_user("admin").await;
    assert!(del_err.is_err(), "Cannot delete the last admin");

    let demote_err = users.set_admin_role("admin", false).await;
    assert!(demote_err.is_err(), "Cannot demote the last admin");

    let bob_id = users
        .create_user("bob", "bob_secure_password_123", true)
        .await
        .unwrap();
    assert!(!bob_id.is_empty());

    assert!(users.set_admin_role("bob", false).await.is_ok());
    assert!(users
        .update_password("bob", "new_bob_pass_456")
        .await
        .is_ok());
    assert!(users.delete_user("bob").await.is_ok());
}

#[tokio::test]
async fn test_config_provenance_and_descriptors() {
    let config = AppConfig::default();
    let provenance = config.get_effective_provenance(None);
    assert!(!provenance.is_empty());
    assert!(provenance
        .iter()
        .any(|e| e.key == "server.port" && e.value == "8080"));
    assert!(provenance
        .iter()
        .any(|e| e.key == "server.host" && e.value == "127.0.0.1"));

    let desc = AppConfig::describe_key("server.port").unwrap();
    assert_eq!(desc.key, "server.port");
    assert_eq!(desc.value_type, "u16");
    assert_eq!(desc.default_value, "8080");

    let val = config.get_by_key_path("server.port").unwrap();
    assert_eq!(val, "8080");
}

#[tokio::test]
async fn test_transfer_cli_service_queries() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("transfer_cli_test.db");
    let db_url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    let pool = init_db(&db_url).await.unwrap();
    let transfers = TransferService::new(Arc::new(SqliteTransferHistoryRepository::new(
        pool.clone(),
    )));

    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO transfer_jobs (id, user_id, name, transfer_type, source_connection_id, source_path,
                destination_connection_id, destination_path, status, phase, transferred_bytes, total_bytes,
                speed_bytes_per_sec, checksum, created_at, updated_at)
         VALUES ('test_job_1', 'user_1', 'Upload Test', 'upload', 'local', '/file.txt', 'local', '/dst.txt',
                 'running', 'transferring', 500, 1000, 50, 'chk', ?, ?)",
    )
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    let job = transfers.get_transfer("test_job_1").await.unwrap();
    assert!(job.is_some());
    let j = job.unwrap();
    assert_eq!(j.name, "Upload Test");
    assert_eq!(j.total_bytes, 1000);
    assert_eq!(j.transferred_bytes, 500);

    let list = transfers
        .list_transfers_filtered(Some("running"), 10, None, None)
        .await
        .unwrap();
    assert_eq!(list.len(), 1);

    let dry_repair = transfers.repair_stuck_transfers(true).await.unwrap();
    assert_eq!(dry_repair, 1);

    let actual_repair = transfers.repair_stuck_transfers(false).await.unwrap();
    assert_eq!(actual_repair, 1);

    let updated_job = transfers
        .get_transfer("test_job_1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated_job.status.as_str(), "failed");

    let dry_purge = transfers.purge_transfers_older_than(0, true).await.unwrap();
    assert_eq!(dry_purge, 1);
}

#[tokio::test]
async fn test_cli_commands_execution() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("cli_test.db");
    let config_path = temp.path().join("config.toml");

    let toml = format!(
        r#"
[server]
host = "127.0.0.1"
port = 8888

[filesystem]
default_local_root = "{}"

[database]
url = "sqlite://{}?mode=rwc"
"#,
        temp.path().join("storage").display(),
        db_path.display()
    );
    fs::write(&config_path, toml).unwrap();

    let cli = backend::cli::Cli {
        config: Some(config_path.clone()),
        json: true,
        quiet: false,
        verbose: false,
        log_level: None,
        command: Some(backend::cli::Commands::Config(
            backend::cli::args::ConfigCommand {
                action: backend::cli::args::ConfigAction::Validate,
            },
        )),
    };
    assert!(backend::cli::run_cli(cli).await.is_ok());

    let cli = backend::cli::Cli {
        config: Some(config_path.clone()),
        json: true,
        quiet: false,
        verbose: false,
        log_level: None,
        command: Some(backend::cli::Commands::Version),
    };
    assert!(backend::cli::run_cli(cli).await.is_ok());

    let cli = backend::cli::Cli {
        config: Some(config_path.clone()),
        json: true,
        quiet: false,
        verbose: false,
        log_level: None,
        command: Some(backend::cli::Commands::Db(backend::cli::args::DbCommand {
            action: backend::cli::args::DbAction::Migrate,
        })),
    };
    assert!(backend::cli::run_cli(cli).await.is_ok());

    let cli = backend::cli::Cli {
        config: Some(config_path.clone()),
        json: true,
        quiet: false,
        verbose: false,
        log_level: None,
        command: Some(backend::cli::Commands::Db(backend::cli::args::DbCommand {
            action: backend::cli::args::DbAction::Integrity,
        })),
    };
    assert!(backend::cli::run_cli(cli).await.is_ok());

    let cli = backend::cli::Cli {
        config: Some(config_path.clone()),
        json: true,
        quiet: false,
        verbose: false,
        log_level: None,
        command: Some(backend::cli::Commands::User(
            backend::cli::args::UserCommand {
                action: backend::cli::args::UserAction::List,
            },
        )),
    };
    assert!(backend::cli::run_cli(cli).await.is_ok());
}
