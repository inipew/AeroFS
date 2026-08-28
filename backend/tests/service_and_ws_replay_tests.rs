use backend::auth::{AuthenticatedUser, UserInfo};
use backend::config::AppConfig;
use backend::db::init_db;
use backend::filesystem::archive::ArchiveOverwriteMode;
use backend::services::{ArchiveService, FileService, TransferService};
use backend::transfer::{TransferType, WsEvent};
use backend::AppState;
use tempfile::tempdir;

async fn setup_test_context() -> (AppState, AuthenticatedUser, tempfile::TempDir) {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("service_test.db");
    let storage_dir = temp.path().join("storage");
    std::fs::create_dir_all(&storage_dir).unwrap();

    let mut config = AppConfig::default();
    config.database.url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    config.filesystem.default_local_root = storage_dir;

    let db = init_db(&config.database.url).await.unwrap();
    let state = AppState::new_with_db(config, db).await;

    let user = AuthenticatedUser(UserInfo {
        id: "admin-id".to_string(),
        username: "admin".to_string(),
        is_admin: true,
    });

    (state, user, temp)
}

#[tokio::test]
async fn test_ws_event_sequence_and_durable_replay() {
    let (state, _, _temp) = setup_test_context().await;

    // 1. Broadcast multiple events
    state.transfer_manager.broadcast_event(WsEvent::FileChange {
        connection_id: "local".into(),
        path: "/file1.txt".into(),
        action: "create".into(),
    });
    state.transfer_manager.broadcast_event(WsEvent::FileChange {
        connection_id: "local".into(),
        path: "/file2.txt".into(),
        action: "write".into(),
    });
    state.transfer_manager.broadcast_event(WsEvent::FileChange {
        connection_id: "local".into(),
        path: "/file3.txt".into(),
        action: "delete".into(),
    });

    // 2. Fetch missed events since sequence 1
    let missed = state.transfer_manager.get_events_since(1).await;
    assert_eq!(missed.len(), 2, "Expected 2 events with sequence > 1");
    assert_eq!(missed[0].sequence, 2);
    assert_eq!(missed[1].sequence, 3);

    // 3. Fetch all events since 0
    let all = state.transfer_manager.get_events_since(0).await;
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].sequence, 1);
    assert_eq!(all[1].sequence, 2);
    assert_eq!(all[2].sequence, 3);
}

#[tokio::test]
async fn test_file_service_full_crud_lifecycle() {
    let (state, user, _temp) = setup_test_context().await;

    // 1. Create directory
    let dir_meta = FileService::create_directory(&state, &user, "local", "/docs")
        .await
        .expect("Directory creation failed");
    assert_eq!(dir_meta.path, "/docs");

    // 2. Create and write file
    let file_meta = FileService::create_or_write_file(
        &state,
        &user,
        "local",
        "/docs/readme.md",
        b"# Hello World".to_vec(),
        None,
    )
    .await
    .expect("File creation failed");
    assert_eq!(file_meta.size, 13);

    // 3. Stat file
    let stat = FileService::stat_file(&state, &user, "local", "/docs/readme.md")
        .await
        .expect("Stat file failed");
    assert_eq!(stat.size, 13);

    // 4. List directory
    let listing = FileService::list_directory(
        &state,
        &user,
        "local",
        Some("/docs".to_string()),
        None,
        None,
        None,
    )
    .await
    .expect("List directory failed");
    assert_eq!(listing.entries.len(), 1);
    assert_eq!(listing.entries[0].name, "readme.md");

    // 5. Rename entry
    FileService::rename_entry(
        &state,
        &user,
        "local",
        "/docs/readme.md",
        "/docs/README_RENAMED.md",
    )
    .await
    .expect("Rename entry failed");

    // 6. Delete entry
    FileService::delete_entry(&state, &user, "local", "/docs/README_RENAMED.md")
        .await
        .expect("Delete entry failed");
}

#[tokio::test]
async fn test_archive_service_lifecycle() {
    let (state, user, _temp) = setup_test_context().await;

    // Create source files
    FileService::create_or_write_file(
        &state,
        &user,
        "local",
        "/src1.txt",
        b"Source file 1 content".to_vec(),
        None,
    )
    .await
    .unwrap();

    FileService::create_or_write_file(
        &state,
        &user,
        "local",
        "/src2.txt",
        b"Source file 2 content".to_vec(),
        None,
    )
    .await
    .unwrap();

    // 1. Compress into ZIP
    let compress_res = ArchiveService::compress(
        &state,
        &user,
        "local",
        "/",
        &["src1.txt".to_string(), "src2.txt".to_string()],
        "/bundle.zip",
        Some("zip"),
    )
    .await
    .expect("Compression failed");
    assert!(compress_res.success);

    // 2. List virtual archive contents
    let virtual_entries = ArchiveService::list_virtual(&state, &user, "local", "/bundle.zip", "")
        .await
        .expect("List virtual archive failed");
    assert!(virtual_entries.iter().any(|e| e.name == "src1.txt"));

    // 3. Read virtual archive entry
    let (filename, bytes) =
        ArchiveService::read_virtual_entry(&state, &user, "local", "/bundle.zip", "src1.txt")
            .await
            .expect("Read virtual archive entry failed");
    assert_eq!(filename, "src1.txt");
    assert_eq!(bytes, b"Source file 1 content");

    // 4. Extract archive
    let extract_res = ArchiveService::extract(
        &state,
        &user,
        "local",
        "/bundle.zip",
        "/extracted",
        Some("zip"),
        ArchiveOverwriteMode::Overwrite,
    )
    .await
    .expect("Extract archive failed");
    assert!(extract_res.success);
}

#[tokio::test]
async fn test_transfer_service_operations() {
    let (state, user, _temp) = setup_test_context().await;

    FileService::create_or_write_file(
        &state,
        &user,
        "local",
        "/transfer_source.txt",
        b"Transfer payload data".to_vec(),
        None,
    )
    .await
    .unwrap();

    // 1. Create transfer
    let job_id = TransferService::create_transfer(
        &state,
        &user,
        "Test Transfer".to_string(),
        TransferType::Copy,
        "local".to_string(),
        "/transfer_source.txt".to_string(),
        "local".to_string(),
        "/transfer_destination.txt".to_string(),
    )
    .await
    .expect("Create transfer failed");
    assert!(!job_id.is_empty());

    // 2. List transfers
    let jobs = TransferService::list_transfers(&state, &user)
        .await
        .expect("List transfers failed");
    assert!(jobs.iter().any(|j| j.id == job_id));

    // Wait a brief moment for transfer to finish or process
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // 3. Dismiss finished transfer
    let _ = TransferService::dismiss_transfer(&state, &user, &job_id).await;
    let _ = TransferService::clear_finished_transfers(&state, &user).await;
}
