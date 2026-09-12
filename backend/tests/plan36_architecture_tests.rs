use axum::extract::FromRef;
use backend::auth::{AuthenticatedUser, UserInfo};
use backend::config::AppConfig;
use backend::db::init_db;
use backend::domain::conflict::{ConflictPolicy, ConflictResolver};
use backend::domain::operation::{FailureStrategy, OperationIntentType, OperationStatus};
use backend::domain::policy::PermissionInheritanceMode;
use backend::domain::settings::UserPreferences;
use backend::domain::{Actor, ConnectionId, VfsPath};
use backend::infrastructure::CredentialStore;
use backend::services::{
    AuditService, AuthService, AuthorizationService, ConnectionService, EditorService, FileService,
    OperationService, PreferencesService, PreviewService, SettingsService, ShareService, TrashService,
};
use backend::state::{HealthState, RuntimePhase, SearchState};
use backend::vfs::factory::ProviderFactory;
use backend::vfs::registry::ProviderRegistry;
use backend::AppState;
use tempfile::tempdir;

async fn get_seeded_admin(db: &backend::db::DbPool) -> AuthenticatedUser {
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

fn connection_service(state: &AppState) -> ConnectionService {
    ConnectionService::new(
        state.db.clone(),
        state.config.clone(),
        state.registry.clone(),
        state.credentials.clone(),
        state.metadata_cache.clone(),
        state.transfer_manager.clone(),
    )
}

async fn setup_test_app() -> (AppState, tempfile::TempDir) {
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

    let state = AppState::new_with_db(config, db).await;

    (state, temp)
}

#[tokio::test]
async fn test_plan36_credential_store_and_provider_factory() {
    let store = CredentialStore::new("test_secret_passphrase_1234567890");
    let secret = "s3_access_secret_key_123456";

    let encrypted = store.encrypt(secret).unwrap();
    assert_ne!(secret, encrypted);

    let decrypted = store.decrypt(&encrypted).unwrap();
    assert_eq!(secret, decrypted);

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
    assert_eq!(registry.get_connection_error("local_1").await, None);
}

#[tokio::test]
async fn test_plan36_connection_service_lifecycle() {
    let (state, temp) = setup_test_app().await;
    let admin = get_seeded_admin(&state.db).await;
    let regular = mock_regular_user();
    let admin_actor = actor(&admin);
    let regular_actor = actor(&regular);
    let service = connection_service(&state);

    let conns = service.list_connections(&admin_actor).await.unwrap();
    assert!(!conns.is_empty());

    let new_storage = temp.path().join("extra_storage");
    std::fs::create_dir_all(&new_storage).unwrap();

    let conn_id = service
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

    let res = service.get_connection(&regular_actor, &conn_id).await;
    assert!(res.is_err());

    let detail = service.get_connection(&admin_actor, &conn_id).await.unwrap();
    assert_eq!(detail.connection.name, "Extra Local");

    let test_res = service
        .test_connection(&admin_actor, &conn_id)
        .await
        .unwrap();
    assert!(test_res.success);

    service
        .delete_connection(&admin_actor, &conn_id)
        .await
        .unwrap();
    assert!(!state.registry.contains(&conn_id).await);
}

#[tokio::test]
async fn test_plan36_settings_and_preferences_services() {
    let (state, _temp) = setup_test_app().await;
    let admin = get_seeded_admin(&state.db).await;

    let settings = SettingsService::get_settings(&state, &admin).await.unwrap();
    assert_eq!(settings.settings.general.theme, "dark");

    let prefs = UserPreferences {
        theme: "dracula".to_string(),
        list_density: "compact".to_string(),
        ..Default::default()
    };

    PreferencesService::set_user_preferences(&state.db, &admin.id, &prefs)
        .await
        .unwrap();
    let fetched = PreferencesService::get_user_preferences(&state.db, &admin.id)
        .await
        .unwrap();
    assert_eq!(fetched.theme, "dracula");
    assert_eq!(fetched.list_density, "compact");

    let new_root = _temp.path().join("switched_root");
    std::fs::create_dir_all(&new_root).unwrap();
    std::fs::write(new_root.join("new_marker.txt"), "hello switched root").unwrap();

    SettingsService::update_settings(
        &state,
        &admin,
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

    let listing = FileService::list_directory(&state, &admin, "local", None, None, None, None)
        .await
        .unwrap();
    assert!(listing.entries.iter().any(|e| e.name == "new_marker.txt"));
}

#[tokio::test]
async fn test_plan36_authorization_service_intent_matrix() {
    let (state, _temp) = setup_test_app().await;
    let regular = mock_regular_user();
    let admin = get_seeded_admin(&state.db).await;

    let res = AuthorizationService::authorize_intent(
        &state.db,
        &admin,
        OperationIntentType::Copy,
        "local",
        Some("local"),
    )
    .await;
    assert!(res.is_ok());

    let res = AuthorizationService::authorize_intent(
        &state.db,
        &regular,
        OperationIntentType::Copy,
        "remote_unauthorized",
        Some("local"),
    )
    .await;
    assert!(res.is_err());
}

#[tokio::test]
async fn test_plan36_conflict_resolver() {
    let fail_res = ConflictResolver::resolve_collision(ConflictPolicy::Fail, "file.txt");
    assert!(fail_res.is_err());

    let skip_res = ConflictResolver::resolve_collision(ConflictPolicy::Skip, "file.txt").unwrap();
    assert_eq!(skip_res, None);

    let replace_res =
        ConflictResolver::resolve_collision(ConflictPolicy::Replace, "file.txt").unwrap();
    assert_eq!(replace_res, Some("file.txt".to_string()));

    let rename_res =
        ConflictResolver::resolve_collision(ConflictPolicy::Rename, "photo.png").unwrap();
    assert_eq!(rename_res, Some("photo_copy.png".to_string()));
}

#[tokio::test]
async fn test_plan36_operation_service_lifecycle() {
    let (state, _temp) = setup_test_app().await;
    let admin = get_seeded_admin(&state.db).await;

    let path = "/test_op.txt";
    FileService::create_or_write_file(
        &state,
        &admin,
        "local",
        path,
        b"Operation Engine Content".to_vec(),
        None,
    )
    .await
    .unwrap();

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

    let exec_res = OperationService::execute_plan(&state, &admin, &plan)
        .await
        .unwrap();
    assert_eq!(exec_res.status, OperationStatus::Completed);
    assert_eq!(exec_res.succeeded_items, vec![path.to_string()]);
}

#[tokio::test]
async fn test_plan36_auth_service_lifecycle() {
    let (state, _temp) = setup_test_app().await;

    let (user_info, session_id) = AuthService::login(&state, "admin", "admin12345", "127.0.0.1")
        .await
        .unwrap();
    assert_eq!(user_info.username, "admin");
    assert!(user_info.is_admin);
    assert!(!session_id.is_empty());

    AuthService::logout(&state, &session_id, Some(&user_info.id), "127.0.0.1")
        .await
        .unwrap();

    let fail_res = AuthService::login(&state, "admin", "wrongpassword", "127.0.0.1").await;
    assert!(fail_res.is_err());
}

#[tokio::test]
async fn test_plan36_specialized_services() {
    let (state, _temp) = setup_test_app().await;
    let admin = get_seeded_admin(&state.db).await;

    // 1. Health now uses a narrow capability instead of AppState.
    state.runtime.set_phase(RuntimePhase::Running);
    let health_state = HealthState::from_ref(&state);
    let health = health_state.service.readiness().await.unwrap();
    assert!(health.active_providers >= 1);
    assert_eq!(health.phase, "running");

    // 2. EditorService
    let edit_path = "/code.rs";
    EditorService::save_from_editing(
        &state,
        &admin,
        "local",
        edit_path,
        "fn main() { println!(\"hello\"); }",
        None,
    )
    .await
    .unwrap();

    let (content, _etag) = EditorService::read_for_editing(&state, &admin, "local", edit_path)
        .await
        .unwrap();
    assert_eq!(content, "fn main() { println!(\"hello\"); }");

    // 3. PreviewService
    let preview_meta = PreviewService::get_preview_info(&state, &admin, "local", edit_path)
        .await
        .unwrap();
    assert_eq!(preview_meta.name, "code.rs");

    // 4. Search now uses SearchState and typed application identity/path inputs.
    let search_state = SearchState::from_ref(&state);
    let actor = Actor {
        id: admin.id.clone(),
        username: admin.username.clone(),
        is_admin: admin.is_admin,
    };
    let connection = ConnectionId::new("local").unwrap();
    let search_out = search_state
        .service
        .search_files(
            &actor,
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

    // 5. TrashService
    let moved_items = TrashService::move_to_trash(
        &state,
        &admin,
        backend::services::trash_service::MoveToTrashRequest {
            connection_id: "local".to_string(),
            paths: vec![edit_path.to_string()],
        },
    )
    .await
    .unwrap();
    assert_eq!(moved_items.len(), 1);

    let trash_items = TrashService::list_trash(&state, &admin).await.unwrap();
    assert_eq!(trash_items.len(), 1);

    TrashService::restore_item(&state, &admin, &trash_items[0].id)
        .await
        .unwrap();

    // 6. ShareService
    let share = ShareService::create_share(
        &state,
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

    let (c_id, p) = ShareService::verify_and_get_public_share(&state, &share.share_token, None)
        .await
        .unwrap();
    assert_eq!(c_id, "local");
    assert_eq!(p, edit_path);

    ShareService::delete_share(&state, &admin, &share.id)
        .await
        .unwrap();

    // 7. AuditService
    AuditService::record(
        &state.db,
        Some(&admin.id),
        "TEST_AUDIT",
        Some("local"),
        Some(edit_path),
        "SUCCESS",
        Some("127.0.0.1"),
        Some("Audit test details"),
    )
    .await;

    let logs = AuditService::list_logs(&state.db, &admin, 10, 0)
        .await
        .unwrap();
    assert!(!logs.is_empty());
}
