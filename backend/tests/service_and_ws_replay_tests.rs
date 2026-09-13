use axum::extract::FromRef;
use backend::auth::{AuthenticatedUser, UserInfo};
use backend::bootstrap::build_application;
use backend::config::AppConfig;
use backend::db::init_db;
use backend::domain::{Actor, ConnectionId};
use backend::events::{DomainEvent, EventJournal, ReplayOutcome};
use backend::filesystem::archive::ArchiveOverwriteMode;
use backend::ports::transfer::TransferType;
use backend::state::{
    ArchiveState, FileApiState, RuntimeOwner, ShutdownReason, TransferState,
};
use backend::AppState;
use tempfile::tempdir;

struct TestRuntime {
    _temp: tempfile::TempDir,
    runtime: RuntimeOwner,
}

impl Drop for TestRuntime {
    fn drop(&mut self) {
        self.runtime.request_shutdown(ShutdownReason::Manual);
    }
}

async fn setup_test_context() -> (AppState, AuthenticatedUser, TestRuntime) {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("service_test.db");
    let storage_dir = temp.path().join("storage");
    std::fs::create_dir_all(&storage_dir).unwrap();

    let mut config = AppConfig::default();
    config.database.url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    config.filesystem.default_local_root = storage_dir;

    let db = init_db(&config.database.url).await.unwrap();
    let built = build_application(config, db).await;

    let user = AuthenticatedUser(UserInfo {
        id: "admin-id".to_string(),
        username: "admin".to_string(),
        is_admin: true,
    });

    (
        built.state,
        user,
        TestRuntime {
            _temp: temp,
            runtime: built.runtime,
        },
    )
}

fn actor_from_user(user: &AuthenticatedUser) -> Actor {
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
) -> backend::domain::FileMetadata {
    let file_api = FileApiState::from_ref(state);
    file_api
        .files
        .write_file
        .execute(
            &actor_from_user(user),
            backend::application::files::WriteFileCommand {
                connection: ConnectionId::local(),
                path: path.to_string(),
                content,
                expected_etag: None,
                create_only: false,
            },
        )
        .await
        .unwrap()
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
async fn test_file_application_full_crud_lifecycle() {
    let (state, user, _runtime) = setup_test_context().await;
    let actor = actor_from_user(&user);
    let connection = ConnectionId::local();
    let file_api = FileApiState::from_ref(&state);

    let dir_meta = file_api
        .files
        .create_directory
        .execute(
            &actor,
            backend::application::files::CreateDirectoryCommand {
                connection: connection.clone(),
                path: "/docs".to_string(),
            },
        )
        .await
        .expect("Directory creation failed");
    assert_eq!(dir_meta.path, "/docs");

    let file_meta = write_file(
        &state,
        &user,
        "/docs/readme.md",
        b"# Hello World".to_vec(),
    )
    .await;
    assert_eq!(file_meta.size, 13);

    let stat = file_api
        .files
        .stat_file
        .execute(
            &actor,
            backend::application::files::StatFileCommand {
                connection: connection.clone(),
                path: "/docs/readme.md".to_string(),
            },
        )
        .await
        .expect("Stat file failed");
    assert_eq!(stat.size, 13);

    let listing = file_api
        .files
        .list_directory
        .execute(
            &actor,
            backend::application::files::ListDirectoryCommand {
                connection: connection.clone(),
                path: Some("/docs".to_string()),
                show_hidden: None,
                sort: None,
                order: None,
                cursor: None,
                limit: None,
            },
        )
        .await
        .expect("List directory failed");
    assert_eq!(listing.entries.len(), 1);
    assert_eq!(listing.entries[0].name, "readme.md");

    file_api
        .files
        .rename_entry
        .execute(
            &actor,
            backend::application::files::RenameEntryCommand {
                connection: connection.clone(),
                from: "/docs/readme.md".to_string(),
                to: "/docs/README_RENAMED.md".to_string(),
            },
        )
        .await
        .expect("Rename entry failed");

    let deleted = file_api
        .files
        .delete_entries
        .execute(
            &actor,
            backend::application::files::DeleteEntriesCommand {
                connection,
                paths: vec!["/docs/README_RENAMED.md".to_string()],
            },
        )
        .await
        .expect("Delete entry failed");
    assert!(deleted.failed.is_empty());
    assert_eq!(deleted.succeeded.len(), 1);
}

#[tokio::test]
async fn test_archive_service_lifecycle() {
    let (state, user, _runtime) = setup_test_context().await;
    let archive = ArchiveState::from_ref(&state);
    let actor = actor_from_user(&user);
    let connection = ConnectionId::local();

    write_file(
        &state,
        &user,
        "/src1.txt",
        b"Source file 1 content".to_vec(),
    )
    .await;

    write_file(
        &state,
        &user,
        "/src2.txt",
        b"Source file 2 content".to_vec(),
    )
    .await;

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
    let (state, user, _runtime) = setup_test_context().await;
    let transfers = TransferState::from_ref(&state);
    let actor = actor_from_user(&user);

    write_file(
        &state,
        &user,
        "/transfer_source.txt",
        b"Transfer payload data".to_vec(),
    )
    .await;

    let job_id = transfers
        .use_cases
        .create_transfer
        .execute(
            &actor,
            backend::application::transfers::CreateTransferCommand {
                name: "Test Transfer".to_string(),
                transfer_type: TransferType::Copy,
                source_connection: ConnectionId::local(),
                source_path: "/transfer_source.txt".to_string(),
                destination_connection: ConnectionId::local(),
                destination_path: "/transfer_destination.txt".to_string(),
            },
        )
        .await
        .expect("Create transfer failed");
    assert!(!job_id.is_empty());

    let jobs = transfers
        .use_cases
        .list(&actor)
        .await
        .expect("List transfers failed");
    assert!(jobs.iter().any(|j| j.id == job_id));

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let _ = transfers.use_cases.dismiss(&actor, &job_id).await;
    let _ = transfers.use_cases.clear_finished(&actor).await;
}
