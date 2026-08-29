use backend::auth::{AuthenticatedUser, UserInfo};
use backend::config::AppConfig;
use backend::db::init_db;
use backend::services::FileService;
use backend::transfer::{ReplayResult, WsEvent};
use backend::AppState;
use tempfile::tempdir;

async fn setup_test_context() -> (AppState, AuthenticatedUser, tempfile::TempDir) {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("plan58_test.db");
    let storage_dir = temp.path().join("storage");
    std::fs::create_dir_all(&storage_dir).unwrap();

    let mut config = AppConfig::default();
    config.database.url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    config.filesystem.default_local_root = storage_dir;

    let db = init_db(&config.database.url).await.unwrap();
    let state = AppState::new_with_db(config, db).await;

    let admin = AuthenticatedUser(UserInfo {
        id: "admin-id".to_string(),
        username: "admin".to_string(),
        is_admin: true,
    });

    (state, admin, temp)
}

#[tokio::test]
async fn test_chmod_emits_file_change_event() {
    let (state, admin, temp) = setup_test_context().await;

    // Create file
    let file_path = temp.path().join("storage").join("test_chmod.txt");
    std::fs::write(&file_path, b"test chmod").unwrap();

    let mut rx = state.transfer_manager.subscribe();

    // Call chmod
    let res = FileService::chmod(&state, &admin, "local", "/test_chmod.txt", 0o755).await;
    assert!(res.is_ok(), "chmod should succeed");

    // Receive broadcast event
    let envelope = rx.recv().await.expect("Expected WsEvent");
    match envelope.event {
        WsEvent::FileChange {
            connection_id,
            path,
            action,
            old_path,
            parent_path,
            ..
        } => {
            assert_eq!(connection_id, "local");
            assert_eq!(path, "/test_chmod.txt");
            assert_eq!(action, "chmod");
            assert!(old_path.is_none());
            assert_eq!(parent_path, Some("/".to_string()));
        }
        other => panic!("Expected WsEvent::FileChange, got {:?}", other),
    }
}

#[tokio::test]
async fn test_rename_emits_source_and_destination_paths() {
    let (state, admin, temp) = setup_test_context().await;

    // Create folders and source file
    let storage = temp.path().join("storage");
    std::fs::create_dir_all(storage.join("folder_a")).unwrap();
    std::fs::create_dir_all(storage.join("folder_b")).unwrap();
    std::fs::write(
        storage.join("folder_a").join("source.txt"),
        b"rename content",
    )
    .unwrap();

    let mut rx = state.transfer_manager.subscribe();

    // Call rename across folders
    let res = FileService::rename_entry(
        &state,
        &admin,
        "local",
        "/folder_a/source.txt",
        "/folder_b/dest.txt",
    )
    .await;
    assert!(res.is_ok(), "rename should succeed");

    // Receive broadcast event
    let envelope = rx.recv().await.expect("Expected WsEvent");
    match envelope.event {
        WsEvent::FileChange {
            connection_id,
            path,
            action,
            old_path,
            parent_path,
            old_parent_path,
        } => {
            assert_eq!(connection_id, "local");
            assert_eq!(path, "/folder_b/dest.txt");
            assert_eq!(action, "rename");
            assert_eq!(old_path, Some("/folder_a/source.txt".to_string()));
            assert_eq!(parent_path, Some("/folder_b".to_string()));
            assert_eq!(old_parent_path, Some("/folder_a".to_string()));
        }
        other => panic!("Expected WsEvent::FileChange, got {:?}", other),
    }
}

#[tokio::test]
async fn test_event_replay_includes_rich_metadata() {
    let (state, _, _temp) = setup_test_context().await;

    // Broadcast rename event
    state.transfer_manager.broadcast_event(WsEvent::file_rename(
        "local",
        "/src/doc.pdf",
        "/archive/doc.pdf",
    ));

    // Fetch replay
    let replay = state.transfer_manager.get_events_since(0).await;
    match replay {
        ReplayResult::Events(events) => {
            assert_eq!(events.len(), 1);
            match &events[0].event {
                WsEvent::FileChange {
                    path,
                    old_path,
                    parent_path,
                    old_parent_path,
                    ..
                } => {
                    assert_eq!(path, "/archive/doc.pdf");
                    assert_eq!(old_path, &Some("/src/doc.pdf".to_string()));
                    assert_eq!(parent_path, &Some("/archive".to_string()));
                    assert_eq!(old_parent_path, &Some("/src".to_string()));
                }
                other => panic!("Expected WsEvent::FileChange, got {:?}", other),
            }
        }
        _ => panic!("Expected Events replay"),
    }
}
