use axum::extract::FromRef;
use backend::auth::{AuthenticatedUser, UserInfo};
use backend::bootstrap::build_application;
use backend::config::AppConfig;
use backend::db::init_db;
use backend::domain::{Actor, ConnectionId};
use backend::ports::transfer::{
    TransferJobResponse, TransferPhase, TransferStatus, TransferType,
};
use backend::services::{EditorService, TransferService};
use backend::state::{AppState, FileApiState, RuntimeOwner, ShutdownReason, TransferState};
use backend::transfer::TransferPhase as EngineTransferPhase;
use std::time::Duration;
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
    let db_path = temp.path().join("plan38_test.db");
    let storage_dir = temp.path().join("storage");
    std::fs::create_dir_all(&storage_dir).unwrap();

    let mut config = AppConfig::default();
    config.database.url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    config.filesystem.default_local_root = storage_dir;

    let db = init_db(&config.database.url).await.unwrap();
    let built = build_application(config, db).await;

    let admin = AuthenticatedUser(UserInfo {
        id: "admin-id".to_string(),
        username: "admin".to_string(),
        is_admin: true,
    });

    (
        built.state,
        admin,
        TestRuntime {
            _temp: temp,
            runtime: built.runtime,
        },
    )
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
                connection: ConnectionId::local(),
                path: path.to_string(),
                content,
                expected_etag: None,
                create_only: false,
            },
        )
        .await
        .unwrap();
}

async fn stat_file(
    state: &AppState,
    user: &AuthenticatedUser,
    path: &str,
) -> Result<backend::domain::FileMetadata, backend::errors::AppError> {
    let file_api = FileApiState::from_ref(state);
    file_api
        .files
        .stat_file
        .execute(
            &actor(user),
            backend::application::files::StatFileCommand {
                connection: ConnectionId::local(),
                path: path.to_string(),
            },
        )
        .await
}

async fn wait_for_completed(
    state: &AppState,
    user: &AuthenticatedUser,
    job_id: &str,
) -> TransferJobResponse {
    let transfers = TransferState::from_ref(state);
    let actor = actor(user);

    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let jobs = transfers.use_cases.list(&actor).await.unwrap();
        if let Some(job) = jobs.into_iter().find(|job| job.id == job_id) {
            if job.status == TransferStatus::Completed {
                return job;
            }
        }
    }

    panic!("transfer job {job_id} did not complete in time");
}

#[test]
fn test_transfer_phase_serialization_and_roundtrip() {
    let phases = vec![
        (EngineTransferPhase::Preparing, "preparing"),
        (EngineTransferPhase::Transferring, "transferring"),
        (EngineTransferPhase::Finalizing, "finalizing"),
        (EngineTransferPhase::Verifying, "verifying"),
        (EngineTransferPhase::CleaningUp, "cleaning_up"),
        (EngineTransferPhase::Completed, "completed"),
    ];

    for (phase, str_val) in phases {
        assert_eq!(phase.as_str(), str_val);
        assert_eq!(EngineTransferPhase::from_str(str_val), phase);
    }
}

#[tokio::test]
async fn test_transfer_phase_transitions_and_completion() {
    let (state, admin, _runtime) = setup_test_context().await;
    let file_api = FileApiState::from_ref(&state);

    let test_data = vec![b'A'; 100 * 1024];
    write_file(
        &state,
        &admin,
        "/source_lifecycle.txt",
        test_data.clone(),
    )
    .await;

    let job_id = TransferService::create_transfer(
        &state,
        &admin,
        "copy_lifecycle".into(),
        TransferType::Copy,
        "local".into(),
        "/source_lifecycle.txt".into(),
        "local".into(),
        "/dest_lifecycle.txt".into(),
    )
    .await
    .unwrap();

    let job = wait_for_completed(&state, &admin, &job_id).await;
    assert_eq!(job.phase, TransferPhase::Completed);
    assert_eq!(job.transferred_bytes, test_data.len() as u64);
    assert_eq!(job.total_bytes, test_data.len() as u64);
    assert!(job.checksum.is_some());

    let edit_res =
        EditorService::read_for_editing(&file_api, &admin, "local", "/dest_lifecycle.txt")
            .await
            .unwrap();
    assert_eq!(edit_res.0.len(), test_data.len());
}

#[tokio::test]
async fn test_move_cleanup_lifecycle() {
    let (state, admin, _runtime) = setup_test_context().await;

    let test_data = b"Transactional Move Lifecycle Test".to_vec();
    write_file(&state, &admin, "/move_source.txt", test_data).await;

    let job_id = TransferService::create_transfer(
        &state,
        &admin,
        "move_lifecycle".into(),
        TransferType::Move,
        "local".into(),
        "/move_source.txt".into(),
        "local".into(),
        "/move_dest.txt".into(),
    )
    .await
    .unwrap();

    let job = wait_for_completed(&state, &admin, &job_id).await;
    assert_eq!(job.phase, TransferPhase::Completed);

    let dest_res = stat_file(&state, &admin, "/move_dest.txt").await;
    assert!(dest_res.is_ok());

    let src_res = stat_file(&state, &admin, "/move_source.txt").await;
    assert!(src_res.is_err(), "Source file should be deleted on move");
}
