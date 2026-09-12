use axum::extract::FromRef;
use backend::auth::{AuthenticatedUser, UserInfo};
use backend::config::AppConfig;
use backend::db::init_db;
use backend::domain::{Actor, ConnectionId};
use backend::events::{DomainEvent, EventJournal, ReplayOutcome};
use backend::filesystem::archive::ArchiveOverwriteMode;
use backend::ports::transfer::TransferType;
use backend::services::{FileService, TransferService};
use backend::state::ArchiveState;
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

fn actor_from_user(user: &AuthenticatedUser) -> Actor {
    Actor {
        id: user.id().to_string(),
        username: user.username().to_string(),
        is_admin: user.is_admin(),
    }
}

#[tokio::test]
async fn test_ws_event_sequence_and_durable_replay() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("journal_replay.db");
    let database_url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    let db = init_db(&database_url).await.unwrap();
    let journal = EventJournal::init(db).await.unwrap();

    journal
        .append(DomainEvent::file_change("local", "/file1.txt", "create"), None)
        .await
        .unwrap();
    journal
        .append(DomainEvent::file_change("local", "/file2.txt", "write"), None)
        .await
        .unwrap();
    journal
        .append(DomainEvent::file_change("local", "/file3.txt", "delete"), None)
        .await
        .unwrap();

    let missed = match journal
        .get_since(Some(journal.epoch()), 1, 100)
        .await
        .unwrap()
    {
        ReplayOutcome::Events(events) => events,
        other => panic!("expected replay events, got {other:?}"),
    };
    assert_eq!(missed.len(), 2, "Expected 2 events with sequence > 1");
    assert_eq!(missed[0].sequence, 2);
    assert_eq!(missed[1].sequence, 3);

    let all = match journal
        .get_since(Some(journal.epoch()), 0, 100)
        .await
        .unwrap()
    {
        ReplayOutcome::Events(events) => events,
        other => panic!("expected replay events, got {other:?}"),
    };
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].sequence, 1);
    assert_eq!(all[1].sequence, 2);
    assert_eq!(all[2].sequence, 3);
}

#[tokio::test]
async fn test_file_service_full_crud_lifecycle() {
    let (state, user, _temp) = setup_test_context().await;

    let dir_meta = FileService::create_directory(&state, &user, "local", "/docs")
        .await
        .expect("Directory creation failed");
    assert_eq!(dir_meta.path, "/docs");

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

    let stat = FileService::stat_file(&state, &user, "local", "/docs/readme.md")
        .await
        .expect("Stat file failed");
    assert_eq!(stat.size, 13);

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

    FileService::rename_entry(
        &state,
        &user,
        "local",
        "/docs/readme.md",
        "/docs/README_RENAMED.md",
    )
    .await
    .expect("Rename entry failed");

    FileService::delete_entry(&state, &user, "local", "/docs/README_RENAMED.md")
        .await
        .expect("Delete entry failed");
}

#[tokio::test]
async fn test_archive_service_lifecycle() {
    let (state, user, _temp) = setup_test_context().await;
    let archive = ArchiveState::from_ref(&state);
    let actor = actor_from_user(&user);
    let connection = ConnectionId::local();

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

    let compress_res = archive
        .service
        .compress(
            &actor,
            &connection,
            "/",
            &["src1.txt".to_string(), "src2.txt".to_string()],
            "/bundle.zip",
            Some("zip"),
        )
        .await
        .expect("Compression failed");
    assert!(compress_res.success);

    let virtual_entries = archive
        .service
        .list_virtual(&actor, &connection, "/bundle.zip", "")
        .await
        .expect("List virtual archive failed");
    assert!(virtual_entries.iter().any(|e| e.name == "src1.txt"));

    let (filename, bytes) = archive
        .service
        .read_virtual_entry(&actor, &connection, "/bundle.zip", "src1.txt")
        .await
        .expect("Read virtual archive entry failed");
    assert_eq!(filename, "src1.txt");
    assert_eq!(bytes, b"Source file 1 content");

    let extract_res = archive
        .service
        .extract(
            &actor,
            &connection,
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

    let jobs = TransferService::list_transfers(&state, &user)
        .await
        .expect("List transfers failed");
    assert!(jobs.iter().any(|j| j.id == job_id));

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let _ = TransferService::dismiss_transfer(&state, &user, &job_id).await;
    let _ = TransferService::clear_finished_transfers(&state, &user).await;
}
