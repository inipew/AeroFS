use axum::extract::FromRef;
use backend::auth::{AuthenticatedUser, UserInfo};
use backend::config::AppConfig;
use backend::db::init_db;
use backend::domain::{Actor, ProviderKind};
use backend::events::DomainEvent;
use backend::services::{CreateConnectionRequest, EditorService, FileService, TransferService};
use backend::state::{ConnectionState, RealtimeState, TransferState};
use backend::transfer::{TransferPhase, TransferStatus, TransferType};
use backend::AppState;
use std::time::Duration;
use tempfile::tempdir;

async fn setup_test_context() -> (AppState, AuthenticatedUser, tempfile::TempDir) {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("plan39_test.db");
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

fn actor(user: &AuthenticatedUser) -> Actor {
    Actor {
        id: user.id().to_string(),
        username: user.username().to_string(),
        is_admin: user.is_admin(),
    }
}

async fn wait_for_status(
    state: &AppState,
    user: &AuthenticatedUser,
    job_id: &str,
    accepted: &[TransferStatus],
) -> Option<backend::transfer::TransferJobResponse> {
    let transfers = TransferState::from_ref(state);
    let actor = actor(user);
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let jobs = transfers.use_cases.list(&actor).await.ok()?;
        if let Some(job) = jobs.into_iter().find(|job| job.id == job_id) {
            if accepted.contains(&job.status) {
                return Some(job);
            }
        }
    }
    None
}

#[tokio::test]
async fn test_realtime_cancellation_with_token() {
    let (state, admin, temp) = setup_test_context().await;

    let test_data = vec![b'X'; 32 * 1024 * 1024];
    std::fs::write(
        temp.path().join("storage").join("source_cancel_test.dat"),
        &test_data,
    )
    .unwrap();

    let realtime = RealtimeState::from_ref(&state);
    let mut events = realtime.service.subscribe();

    let job_id = TransferService::create_transfer(
        &state,
        &admin,
        "cancel_job_test".into(),
        TransferType::Copy,
        "local".into(),
        "/source_cancel_test.dat".into(),
        "local".into(),
        "/dest_cancel_test.dat".into(),
    )
    .await
    .unwrap();

    TransferService::cancel_transfer(&state, &admin, &job_id)
        .await
        .unwrap();

    let cancelled = tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let envelope = events.recv().await.expect("realtime event stream should stay open");
            if let DomainEvent::TransferCancelled(value) = envelope.event {
                if value.get("id").and_then(|id| id.as_str()) == Some(job_id.as_str()) {
                    break true;
                }
            }
        }
    })
    .await
    .unwrap_or(false);

    assert!(cancelled, "transfer job should emit durable cancellation event");

    let part_path = format!("/.dest_cancel_test.dat.aerofs-part-{job_id}");
    let part_stat = FileService::stat_file(&state, &admin, "local", &part_path).await;
    assert!(
        part_stat.is_err(),
        "Staging part file should be deleted on cancellation"
    );
}

#[tokio::test]
async fn test_directory_transfer_bounded_limits_and_creation() {
    let (state, admin, _temp) = setup_test_context().await;

    FileService::create_directory(&state, &admin, "local", "/dir_source")
        .await
        .unwrap();
    FileService::create_directory(&state, &admin, "local", "/dir_source/nested")
        .await
        .unwrap();
    FileService::create_or_write_file(
        &state,
        &admin,
        "local",
        "/dir_source/file1.txt",
        b"Content 1".to_vec(),
        None,
    )
    .await
    .unwrap();
    FileService::create_or_write_file(
        &state,
        &admin,
        "local",
        "/dir_source/nested/file2.txt",
        b"Content 2".to_vec(),
        None,
    )
    .await
    .unwrap();

    let job_id = TransferService::create_transfer(
        &state,
        &admin,
        "dir_copy_test".into(),
        TransferType::Copy,
        "local".into(),
        "/dir_source".into(),
        "local".into(),
        "/dir_dest".into(),
    )
    .await
    .unwrap();

    let job = wait_for_status(&state, &admin, &job_id, &[TransferStatus::Completed])
        .await
        .expect("directory transfer did not complete in time");
    assert_eq!(job.phase, TransferPhase::Completed);

    let f1 = EditorService::read_for_editing(&state, &admin, "local", "/dir_dest/file1.txt")
        .await
        .unwrap();
    assert_eq!(f1.0, "Content 1");

    let f2 = EditorService::read_for_editing(&state, &admin, "local", "/dir_dest/nested/file2.txt")
        .await
        .unwrap();
    assert_eq!(f2.0, "Content 2");
}

#[tokio::test]
async fn test_connection_deletion_drains_active_transfers() {
    let (state, admin, temp) = setup_test_context().await;
    let connections = ConnectionState::from_ref(&state);
    let admin_actor = actor(&admin);

    let remote_dir = temp.path().join("dummy_remote");
    std::fs::create_dir_all(&remote_dir).unwrap();

    let conn_id = connections
        .service
        .create_connection(
            &admin_actor,
            CreateConnectionRequest {
                name: "Dummy Remote".to_string(),
                provider: ProviderKind::Local,
                host: None,
                port: None,
                username: None,
                secret: None,
                base_path: Some(remote_dir.to_str().unwrap().to_string()),
                read_only: None,
            },
        )
        .await
        .unwrap();

    let test_data = vec![b'Z'; 5 * 1024 * 1024];
    FileService::create_or_write_file(
        &state,
        &admin,
        "local",
        "/drain_source.dat",
        test_data,
        None,
    )
    .await
    .unwrap();

    let job_id = TransferService::create_transfer(
        &state,
        &admin,
        "drain_test".into(),
        TransferType::Copy,
        "local".into(),
        "/drain_source.dat".into(),
        conn_id.clone(),
        "/drain_dest.dat".into(),
    )
    .await
    .unwrap();

    connections
        .service
        .delete_connection(&admin_actor, &conn_id)
        .await
        .unwrap();

    let job = wait_for_status(
        &state,
        &admin,
        &job_id,
        &[
            TransferStatus::Cancelled,
            TransferStatus::Failed,
            TransferStatus::CancellationRequested,
        ],
    )
    .await
    .expect("transfer should settle after connection deletion");

    assert!(matches!(
        job.status,
        TransferStatus::Cancelled
            | TransferStatus::Failed
            | TransferStatus::CancellationRequested
    ));
}
