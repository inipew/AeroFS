use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use backend::config::AppConfig;
use backend::db::{backup_db, check_integrity, init_db, vacuum_db};
use backend::{create_router, AppState};
use serde_json::json;
use std::fs;
use tempfile::tempdir;
use tower::ServiceExt;

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

    // 5. Test online backup snapshot helper
    let backup_path = temp.path().join("backups/snapshot.db");
    assert!(backup_db(&pool, &backup_path).await.is_ok());
    assert!(backup_path.exists());
}

#[tokio::test]
async fn test_settings_transaction_and_audit() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("settings_test.db");
    let storage_dir = temp.path().join("storage");
    fs::create_dir_all(&storage_dir).unwrap();

    let mut config = AppConfig::default();
    config.database.url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    config.filesystem.default_local_root = storage_dir;

    let db = init_db(&config.database.url).await.unwrap();
    let state = AppState::new_with_db(config, db).await;
    let app = create_router(state.clone());

    // 1. Login as admin
    let login_req = Request::builder()
        .uri("/api/v1/auth/login")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "username": "admin", "password": "admin12345" }).to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(login_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let cookie = resp
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    // 2. Update settings via PUT /api/v1/settings
    let update_req = Request::builder()
        .uri("/api/v1/settings")
        .method("PUT")
        .header(header::COOKIE, &cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "settings": {
                    "general": {
                        "language": "en",
                        "theme": "dark",
                        "default_view": "list",
                        "default_sort": "size",
                        "sort_direction": "desc",
                        "show_hidden_default": true,
                        "confirm_destructive": true
                    },
                    "file_manager": {
                        "default_layout": "split",
                        "show_breadcrumbs": true,
                        "show_file_size": true,
                        "show_permissions": true,
                        "remember_last_directories": true
                    },
                    "transfers": {
                        "max_concurrent_transfers": 6,
                        "retry_attempts": 5,
                        "auto_retry": true,
                        "show_notifications": true
                    },
                    "connections": {
                        "connection_timeout_secs": 120,
                        "health_check_interval_secs": 30,
                        "auto_reconnect": true,
                        "default_local_root": "",
                        "temp_dir": ""
                    },
                    "security": {
                        "allow_symlinks_outside_root": false,
                        "confirm_permanent_delete": true,
                        "read_only_default": false,
                        "session_timeout_secs": 86400
                    },
                    "advanced": {
                        "log_level": "debug",
                        "enable_telemetry": false,
                        "enable_tracing": true,
                        "directory_cache_ttl_secs": 0
                    }
                }
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(update_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 3. Verify audit log entry was recorded for SETTINGS_UPDATED
    let audit_rows: Vec<(String, String)> =
        sqlx::query_as("SELECT action, status FROM audit_logs WHERE action = 'SETTINGS_UPDATED'")
            .fetch_all(&state.db)
            .await
            .unwrap();

    assert_eq!(audit_rows.len(), 1);
    assert_eq!(audit_rows[0].0, "SETTINGS_UPDATED");
    assert_eq!(audit_rows[0].1, "success");
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
        command: Some(backend::cli::Commands::Config(
            backend::cli::ConfigCommand {
                action: backend::cli::ConfigAction::Validate,
            },
        )),
    };
    assert!(backend::cli::run_cli(cli).await.is_ok());

    // 2. Test CLI Doctor
    let cli = backend::cli::Cli {
        config: Some(config_path.clone()),
        json: true,
        command: Some(backend::cli::Commands::Doctor(backend::cli::DoctorArgs {
            repair: false,
        })),
    };
    assert!(backend::cli::run_cli(cli).await.is_ok());

    // 3. Test CLI Db Status & Integrity Check
    let cli = backend::cli::Cli {
        config: Some(config_path.clone()),
        json: true,
        command: Some(backend::cli::Commands::Db(backend::cli::DbCommand {
            action: backend::cli::DbAction::IntegrityCheck,
        })),
    };
    assert!(backend::cli::run_cli(cli).await.is_ok());

    // 4. Test CLI Admin User Create
    let cli = backend::cli::Cli {
        config: Some(config_path.clone()),
        json: true,
        command: Some(backend::cli::Commands::Admin(backend::cli::AdminCommand {
            action: backend::cli::AdminAction::User(backend::cli::UserCommand {
                action: backend::cli::UserAction::Create {
                    username: "operator".to_string(),
                    password: Some("operator123".to_string()),
                    admin: false,
                },
            }),
        })),
    };
    assert!(backend::cli::run_cli(cli).await.is_ok());
}

#[tokio::test]
async fn test_user_preferences_api() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("prefs_test.db");
    let storage_dir = temp.path().join("storage");
    fs::create_dir_all(&storage_dir).unwrap();

    let mut config = AppConfig::default();
    config.database.url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    config.filesystem.default_local_root = storage_dir;

    let db = init_db(&config.database.url).await.unwrap();
    let state = AppState::new_with_db(config, db).await;
    let app = create_router(state.clone());

    // 1. Login as admin
    let login_req = Request::builder()
        .uri("/api/v1/auth/login")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "username": "admin", "password": "admin12345" }).to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(login_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let cookie = resp
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    // 2. GET /api/v1/user/preferences -> returns defaults
    let req = Request::builder()
        .uri("/api/v1/user/preferences")
        .method("GET")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 3. PUT /api/v1/user/preferences -> updates preferences
    let update_req = Request::builder()
        .uri("/api/v1/user/preferences")
        .method("PUT")
        .header(header::COOKIE, &cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "language": "id",
                "theme": "light",
                "default_view": "list",
                "default_sort": "size",
                "sort_direction": "desc",
                "show_hidden": true,
                "confirm_destructive": false,
                "default_layout": "single",
                "show_breadcrumbs": true,
                "show_file_size": true,
                "show_permissions": true,
                "remember_last_directories": false
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(update_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 4. GET /api/v1/user/preferences -> verifies persisted values
    let req = Request::builder()
        .uri("/api/v1/user/preferences")
        .method("GET")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let val: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(val["theme"], "light");
    assert_eq!(val["default_view"], "list");
    assert_eq!(val["language"], "id");
    assert_eq!(val["default_layout"], "single");
}
