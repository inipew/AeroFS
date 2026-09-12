use axum::extract::FromRef;
use backend::auth::{AuthenticatedUser, UserInfo};
use backend::bootstrap::build_application;
use backend::config::AppConfig;
use backend::db::{init_db, DbPool};
use backend::domain::conflict::{ConflictPolicy, ConflictResolver};
use backend::domain::operation::{FailureStrategy, OperationIntentType, OperationStatus};
use backend::domain::policy::PermissionInheritanceMode;
use backend::domain::settings::UserPreferences;
use backend::domain::{Actor, ConnectionId, VfsPath};
use backend::infrastructure::CredentialStore;
use backend::services::{EditorService, OperationService, PreviewService};
use backend::state::{
    AuditState, AuthState, ConnectionState, FileApiState, HealthState, PreferencesState,
    RuntimeOwner, RuntimePhase, SearchState, SettingsState, ShareState, TrashState,
};
use backend::vfs::factory::ProviderFactory;
use backend::vfs::registry::ProviderRegistry;
use backend::AppState;
use tempfile::tempdir;

struct TestApp {
    state: AppState,
    runtime: RuntimeOwner,
    db: DbPool,
    temp: tempfile::TempDir,
}

async fn get_seeded_admin(db: &DbPool) -> AuthenticatedUser {
    let row: (String, String) =
        sqlx::query_as("SELECT id, username FROM users WHERE username = 'admin'")
            .fetch_one(db)
            .await
            .unwrap();
    AuthenticatedUser(UserInfo {
        id: row.0,
        username: row.1,
        is_admin: true,
    })
}

fn mock_regular_user() -> AuthenticatedUser {
    AuthenticatedUser(UserInfo {
        id: "user_regular".to_string(),
        username: "dhimas".to_string(),
        is_admin: false,
    })
}

fn actor(user: &AuthenticatedUser) -> Actor {
    Actor {
        id: user.id().to_string(),
        username: user.username().to_string(),
        is_admin: user.is_admin(),
    }
}

async fn write_file(
    state: &AppState,
    user: &AuthenticatedUser,
    path: &str,
    content: Vec<u8>,
) {
    let file_api = FileApiState::from_ref(state);
    file_api
        .files
        .write_file
        .execute(
            &actor(user),
            backend::application::files::WriteFileCommand {
                connection: ConnectionId::new("local").unwrap(),
                path: path.to_string(),
                content,
                expected_etag: None,
                create_only: false,
            },
        )
        .await
        .unwrap();
}

async fn setup_test_app() -> TestApp {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("arch_test.db");
    let storage_dir = temp.path().join("storage");
    std::fs::create_dir_all(&storage_dir).unwrap();

    let mut config = AppConfig::default();
    config.database.url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    config.filesystem.default_local_root = storage_dir;
    config.security.session_secret = "super_secret_for_tests_1234567890123456".to_string();

    let db = init_db(&config.database.url).await.unwrap();
    let now = chrono::Utc::now().to_rfc3339();
    let _ = sqlx::query("INSERT OR IGNORE INTO users (id, username, password_hash, is_admin, created_at, updated_at) VALUES ('user_regular', 'dhimas', 'dummy_hash', 0, ?, ?)")
        .bind(&now)
        .bind(&now)
        .execute(&db)
        .await;

    let built = build_application(config, db.clone()).await;
    TestApp {
        state: built.state,
        runtime: built.runtime,
        db,
        temp,
    }
}

#[tokio::test]
async fn test_plan36_credential_store_and_provider_factory() {
    let store = CredentialStore::new("test_secret_passphrase_1234567890");
    let secret = "s3_access_secret_key_123456";
    let encrypted = store.encrypt(secret).unwrap();
    assert_ne!(secret, encrypted);
    assert_eq!(store.decrypt(&encrypted).unwrap(), secret);

    let temp = tempdir().unwrap();
    let local_fs = ProviderFactory::build_local("test_local", temp.path().to_path_buf()).unwrap();
    assert!(local_fs.capabilities().read);
}

#[tokio::test]
async fn test_plan36_provider_registry_lifecycle() {
    let registry = ProviderRegistry::new();
    let temp = tempdir().unwrap();
    let local_fs = ProviderFactory::build_local("local_1", temp.path().to_path_buf()).unwrap();

    assert!(!registry.contains("local_1").await);
    registry.register("local_1".to_string(), local_fs).await;
    assert!(registry.contains("local_1").await);
    assert_eq!(registry.list_ids().await, vec!["local_1"]);
    registry
        .set_connection_error("local_1", "Connection timed out")
        .await;
    assert_eq!(
        registry.get_connection_error("local_1").await,
        Some("Connection timed out".to_string())
    );
    registry.remove("local_1").await;
    assert!(!registry.contains("local_1").await);
}

#[tokio::test]
async fn test_plan36_connection_service_lifecycle() {
    let app = setup_test_app().await;
    let admin = get_seeded_admin(&app.db).await;
    let regular = mock_regular_user();
    let admin_actor = actor(&admin);
    let regular_actor = actor(&regular);
    let connection_state = ConnectionState::from_ref(&app.state);

    let conns = connection_state.service.list_connections(&admin_actor).await.unwrap();
    assert!(!conns.is_empty());

    let new_storage = app.temp.path().join("extra_storage");
    std::fs::create_dir_all(&new_storage).unwrap();
    let conn_id = connection_state
        .service
        .create_connection(
            &admin_actor,
            backend::services::connection_service::CreateConnectionRequest {
                name: "Extra Local".to_string(),
                provider: backend::domain::ProviderKind::Local,
                host: None,
                port: None,
                username: None,
                secret: None,
                base_path: Some(new_storage.to_string_lossy().to_string()),
                read_only: Some(false),
            },
        )
        .await
        .unwrap();

    assert!(connection_state
        .service
        .get_connection(&regular_actor, &conn_id)
        .await
        .is_err());
    let detail = connection_state
        .service
        .get_connection(&admin_actor, &conn_id)
        .await
        .unwrap();
    assert_eq!(detail.connection.name, "Extra Local");
    assert!(connection_state
        .service
        .test_connection(&admin_actor, &conn_id)
        .await
        .unwrap()
        .success);
    connection_state
        .service
        .delete_connection(&admin_actor, &conn_id)
        .await
        .unwrap();
    assert!(connection_state
        .service
        .get_connection(&admin_actor, &conn_id)
        .await
        .is_err());
}

#[tokio::test]
async fn test_plan36_settings_and_preferences_services() {
    let app = setup_test_app().await;
    let admin = get_seeded_admin(&app.db).await;
    let admin_actor = actor(&admin);
    let settings_state = SettingsState::from_ref(&app.state);
    let preferences = PreferencesState::from_ref(&app.state);

    let settings = settings_state
        .service
        .get_settings(&admin_actor)
        .await
        .unwrap();
    assert_eq!(settings.settings.general.theme, "dark");

    let prefs = UserPreferences {
        theme: "dracula".to_string(),
        list_density: "compact".to_string(),
        ..Default::default()
    };
    preferences
        .service
        .set_user_preferences(&admin.id, &prefs)
        .await
        .unwrap();
    let fetched = preferences
        .service
        .get_user_preferences(&admin.id)
        .await
        .unwrap();
    assert_eq!(fetched.theme, "dracula");
    assert_eq!(fetched.list_density, "compact");

    let new_root = app.temp.path().join("switched_root");
    std::fs::create_dir_all(&new_root).unwrap();
    std::fs::write(new_root.join("new_marker.txt"), "hello switched root").unwrap();
    settings_state
        .service
        .update_settings(
            &admin_actor,
            backend::services::settings_service::UpdateSettingsRequest {
                settings: None,
                local_root: Some(new_root.to_string_lossy().to_string()),
                temp_dir: None,
                allow_symlinks: None,
                show_hidden_default: None,
                read_only_default: None,
            },
        )
        .await
        .unwrap();

    let file_api = FileApiState::from_ref(&app.state);
    let listing = file_api
        .files
        .list_directory
        .execute(
            &admin_actor,
            backend::application::files::ListDirectoryCommand {
                connection: ConnectionId::new("local").unwrap(),
                path: None,
                show_hidden: None,
                sort: None,
                order: None,
                cursor: None,
                limit: None,
            },
        )
        .await
        .unwrap();
    assert!(listing.entries.iter().any(|e| e.name == "new_marker.txt"));
}

#[tokio::test]
async fn test_plan36_authorization_service_intent_matrix() {
    let app = setup_test_app().await;
    let regular = mock_regular_user();
    let admin = get_seeded_admin(&app.db).await;
    let file_api = FileApiState::from_ref(&app.state);
    let local = ConnectionId::new("local").unwrap();
    let unauthorized = ConnectionId::new("remote_unauthorized").unwrap();

    assert!(file_api
        .service
        .authorize_intent(&actor(&admin), OperationIntentType::Copy, &local, Some(&local))
        .await
        .is_ok());
    assert!(file_api
        .service
        .authorize_intent(
            &actor(&regular),
            OperationIntentType::Copy,
            &unauthorized,
            Some(&local),
        )
        .await
        .is_err());
}

#[tokio::test]
async fn test_plan36_conflict_resolver() {
    assert!(ConflictResolver::resolve_collision(ConflictPolicy::Fail, "file.txt").is_err());
    assert_eq!(
        ConflictResolver::resolve_collision(ConflictPolicy::Skip, "file.txt").unwrap(),
        None
    );
    assert_eq!(
        ConflictResolver::resolve_collision(ConflictPolicy::Replace, "file.txt").unwrap(),
        Some("file.txt".to_string())
    );
    assert_eq!(
        ConflictResolver::resolve_collision(ConflictPolicy::Rename, "photo.png").unwrap(),
        Some("photo_copy.png".to_string())
    );
}

#[tokio::test]
async fn test_plan36_operation_service_lifecycle() {
    let app = setup_test_app().await;
    let admin = get_seeded_admin(&app.db).await;
    let file_api = FileApiState::from_ref(&app.state);
    let path = "/test_op.txt";
    write_file(
        &app.state,
        &admin,
        path,
        b"Operation Engine Content".to_vec(),
    )
    .await;

    let plan = OperationService::create_plan(
        OperationIntentType::Delete,
        "local".to_string(),
        vec![VfsPath::new("local", path).unwrap()],
        None,
        None,
        FailureStrategy::ContinueOnFailure,
        PermissionInheritanceMode::InheritParent,
        None,
    );
    let exec_res = OperationService::execute_plan(&file_api, &admin, &plan)
        .await
        .unwrap();
    assert_eq!(exec_res.status, OperationStatus::Completed);
    assert_eq!(exec_res.succeeded_items, vec![path.to_string()]);
}

#[tokio::test]
async fn test_plan36_auth_service_lifecycle() {
    let app = setup_test_app().await;
    let auth = AuthState::from_ref(&app.state);
    let (user_info, session_id) = auth
        .service
        .login("admin", "admin12345", "127.0.0.1")
        .await
        .unwrap();
    assert_eq!(user_info.username, "admin");
    assert!(user_info.is_admin);
    assert!(!session_id.is_empty());
    auth.service
        .logout(&session_id, Some(&user_info.id), "127.0.0.1")
        .await
        .unwrap();
    assert!(auth
        .service
        .login("admin", "wrongpassword", "127.0.0.1")
        .await
        .is_err());
}

#[tokio::test]
async fn test_plan36_specialized_services() {
    let app = setup_test_app().await;
    let admin = get_seeded_admin(&app.db).await;
    let file_api = FileApiState::from_ref(&app.state);

    app.runtime.set_phase(RuntimePhase::Running);
    let health = HealthState::from_ref(&app.state)
        .service
        .readiness()
        .await
        .unwrap();
    assert!(health.active_providers >= 1);
    assert_eq!(health.phase, "running");

    let edit_path = "/code.rs";
    EditorService::save_from_editing(
        &file_api,
        &admin,
        "local",
        edit_path,
        "fn main() { println!(\"hello\"); }",
        None,
    )
    .await
    .unwrap();
    let (content, _etag) = EditorService::read_for_editing(&file_api, &admin, "local", edit_path)
        .await
        .unwrap();
    assert_eq!(content, "fn main() { println!(\"hello\"); }");
    let preview_meta = PreviewService::get_preview_info(&file_api, &admin, "local", edit_path)
        .await
        .unwrap();
    assert_eq!(preview_meta.name, "code.rs");

    let search_state = SearchState::from_ref(&app.state);
    let connection = ConnectionId::new("local").unwrap();
    let search_out = search_state
        .service
        .search_files(
            &actor(&admin),
            &connection,
            Some("/"),
            "code",
            false,
            Some(5),
            Some(10),
        )
        .await
        .unwrap();
    assert!(!search_out.results.is_empty());

    let trash = TrashState::from_ref(&app.state);
    let moved_items = trash
        .service
        .move_to_trash(
            &admin,
            backend::services::trash_service::MoveToTrashRequest {
                connection_id: "local".to_string(),
                paths: vec![edit_path.to_string()],
            },
        )
        .await
        .unwrap();
    assert_eq!(moved_items.len(), 1);
    let trash_items = trash.service.list_trash(&admin).await.unwrap();
    assert_eq!(trash_items.len(), 1);
    trash
        .service
        .restore_item(&admin, &trash_items[0].id)
        .await
        .unwrap();

    let shares = ShareState::from_ref(&app.state);
    let share = shares
        .service
        .create_share(
            &admin,
            backend::services::share_service::CreateShareRequest {
                connection_id: "local".to_string(),
                path: edit_path.to_string(),
                password: None,
                expires_in_hours: Some(24),
            },
        )
        .await
        .unwrap();
    let (c_id, p) = shares
        .service
        .verify_and_get_public_share(&share.share_token, None)
        .await
        .unwrap();
    assert_eq!(c_id, "local");
    assert_eq!(p, edit_path);
    shares
        .service
        .delete_share(&admin, &share.id)
        .await
        .unwrap();

    let audit = AuditState::from_ref(&app.state);
    audit
        .service
        .record(
            Some(&admin.id),
            "TEST_AUDIT",
            Some("local"),
            Some(edit_path),
            "SUCCESS",
            Some("127.0.0.1"),
            Some("Audit test details"),
        )
        .await;
    let logs = audit.service.list_logs(&admin, 10, 0).await.unwrap();
    assert!(!logs.is_empty());
}
