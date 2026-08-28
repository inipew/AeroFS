use backend::cli::daemon_lock::{DaemonLock, ProcessStatus};
use backend::config::AppConfig;
use backend::db::{
    backup_db, check_integrity, checkpoint_db, connect_db, get_db_stats, init_db, migrate_db,
    vacuum_db,
};
use backend::services::user_service::UserService;
use backend::services::TransferService;
use std::fs;
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

    // 1. Test load from explicit TOML path
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

    // 2. Test sanitized TOML output (masks secrets)
    let sanitized = config.to_sanitized_toml();
    assert!(sanitized.contains("********"));
    assert!(!sanitized.contains("custom_secret_key_that_is_long_enough"));

    // 3. Test validation rejection (port 0)
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

    // 1. Verify PRAGMA journal_mode is WAL
    let row: (String,) = sqlx::query_as("PRAGMA journal_mode;")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.0.to_lowercase(), "wal");

    // 2. Verify PRAGMA foreign_keys is ON (1)
    let row_fk: (i64,) = sqlx::query_as("PRAGMA foreign_keys;")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row_fk.0, 1);

    // 3. Verify check_integrity helper passes
    let reports = check_integrity(&pool).await.unwrap();
    assert!(reports.iter().any(|r| r.contains("integrity_check: ok")));
    assert!(reports.iter().any(|r| r.contains("foreign_key_check: ok")));

    // 4. Test VACUUM helper
    assert!(vacuum_db(&pool).await.is_ok());

    // 5. Test Checkpoint helper
    assert!(checkpoint_db(&pool).await.is_ok());

    // 6. Test online backup snapshot helper
    let backup_path = temp.path().join("backups/snapshot.db");
    assert!(backup_db(&pool, &backup_path, false).await.is_ok());
    assert!(backup_path.exists());

    // Test overwrite prevention without force
    assert!(backup_db(&pool, &backup_path, false).await.is_err());
    // Test overwrite with force
    assert!(backup_db(&pool, &backup_path, true).await.is_ok());
}

#[tokio::test]
async fn test_db_isolation_and_stats() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("isolation_test.db");
    let db_url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());

    // connect_db should NOT run migrations or create tables
    let pool = connect_db(&db_url).await.unwrap();

    let table_exists: Option<(String,)> =
        sqlx::query_as("SELECT name FROM sqlite_master WHERE type='table' AND name='users'")
            .fetch_optional(&pool)
            .await
            .unwrap();

    assert!(
        table_exists.is_none(),
        "connect_db must not create tables or run migrations"
    );

    // Now explicitly run migrate_db
    let applied = migrate_db(&pool).await.unwrap();
    assert!(!applied.is_empty(), "migrate_db should apply migrations");

    // get_db_stats test
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

    // Initial state: Stopped
    let status = DaemonLock::inspect_status(&lock_path, "127.0.0.1", 8080);
    assert_eq!(status, ProcessStatus::Stopped);

    // Acquire lock
    let lock1 = DaemonLock::acquire(&lock_path).unwrap();
    assert!(lock_path.exists());

    // Second acquire must fail with already running
    assert!(DaemonLock::acquire(&lock_path).is_err());

    // Inspect status while lock held
    let running_status = DaemonLock::inspect_status(&lock_path, "127.0.0.1", 8080);
    match running_status {
        ProcessStatus::Running { pid, .. } => {
            assert_eq!(pid, std::process::id());
        }
        other => panic!("Expected Running status, got: {:?}", other),
    }

    // Release lock
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

    // Default admin was seeded
    let users = UserService::list_users(&pool).await.unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].username, "admin");
    assert!(users[0].is_admin);

    // 1. Attempting to delete the only admin MUST fail
    let del_err = UserService::delete_user(&pool, "admin").await;
    assert!(del_err.is_err(), "Cannot delete the last admin");

    // 2. Attempting to demote the only admin MUST fail
    let demote_err = UserService::set_admin_role(&pool, "admin", false).await;
    assert!(demote_err.is_err(), "Cannot demote the last admin");

    // 3. Create a second admin
    let bob_id = UserService::create_user(&pool, "bob", "bob_secure_password_123", true)
        .await
        .unwrap();
    assert!(!bob_id.is_empty());

    // 4. Now demoting or deleting one admin should succeed because another remains
    assert!(UserService::set_admin_role(&pool, "bob", false)
        .await
        .is_ok());

    // 5. Updating password
    assert!(
        UserService::update_password(&pool, "bob", "new_bob_pass_456")
            .await
            .is_ok()
    );

    // 6. Delete bob
    assert!(UserService::delete_user(&pool, "bob").await.is_ok());
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

    // Descriptors
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

    // Insert dummy transfer job
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

    // Test get_transfer
    let job = TransferService::get_transfer(&pool, "test_job_1")
        .await
        .unwrap();
    assert!(job.is_some());
    let j = job.unwrap();
    assert_eq!(j.name, "Upload Test");
    assert_eq!(j.total_bytes, 1000);
    assert_eq!(j.transferred_bytes, 500);

    // Test list_transfers_filtered
    let list = TransferService::list_transfers_filtered(&pool, Some("running"), 10, None, None)
        .await
        .unwrap();
    assert_eq!(list.len(), 1);

    // Test repair_stuck_transfers
    let dry_repair = TransferService::repair_stuck_transfers(&pool, true)
        .await
        .unwrap();
    assert_eq!(dry_repair, 1);

    let actual_repair = TransferService::repair_stuck_transfers(&pool, false)
        .await
        .unwrap();
    assert_eq!(actual_repair, 1);

    let updated_job = TransferService::get_transfer(&pool, "test_job_1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated_job.status.as_str(), "failed");

    // Test purge dry-run
    let dry_purge = TransferService::purge_transfers_older_than(&pool, 0, true)
        .await
        .unwrap();
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

    // 1. Test CLI Config Validate
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

    // 2. Test CLI Version
    let cli = backend::cli::Cli {
        config: Some(config_path.clone()),
        json: true,
        quiet: false,
        verbose: false,
        log_level: None,
        command: Some(backend::cli::Commands::Version),
    };
    assert!(backend::cli::run_cli(cli).await.is_ok());

    // 3. Test CLI Db Migrate (run migrations so tables exist)
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

    // 4. Test CLI Db Integrity Check
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

    // 5. Test CLI User List
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
