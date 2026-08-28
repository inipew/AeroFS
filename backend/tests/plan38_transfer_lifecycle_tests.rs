use backend::auth::{AuthenticatedUser, UserInfo};
use backend::config::AppConfig;
use backend::db::init_db;
use backend::services::{EditorService, FileService, TransferService};
use backend::transfer::{TransferPhase, TransferStatus, TransferType};
use backend::AppState;
use std::time::Duration;
use tempfile::tempdir;

async fn setup_test_context() -> (AppState, AuthenticatedUser, tempfile::TempDir) {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("plan38_test.db");
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

#[test]
fn test_transfer_phase_serialization_and_roundtrip() {
    let phases = vec![
        (TransferPhase::Preparing, "preparing"),
        (TransferPhase::Transferring, "transferring"),
        (TransferPhase::Finalizing, "finalizing"),
        (TransferPhase::Verifying, "verifying"),
        (TransferPhase::CleaningUp, "cleaning_up"),
        (TransferPhase::Completed, "completed"),
    ];

    for (phase, str_val) in phases {
        assert_eq!(phase.as_str(), str_val);
        assert_eq!(TransferPhase::from_str(str_val), phase);
    }
}

#[tokio::test]
async fn test_transfer_phase_transitions_and_completion() {
    let (state, admin, _temp) = setup_test_context().await;

    // 1. Create source file (100 KB)
    let test_data = vec![b'A'; 100 * 1024];
    FileService::create_or_write_file(
        &state,
        &admin,
        "local",
        "/source_lifecycle.txt",
        test_data.clone(),
        None,
    )
    .await
    .unwrap();

    // 2. Submit copy transfer
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

    // 3. Wait for transfer engine to process
    let mut completed = false;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let jobs = state
            .transfer_manager
            .list_jobs(Some(&admin.id), true, false)
            .await;
        if let Some(j) = jobs.iter().find(|j| j.id == job_id) {
            if j.status == TransferStatus::Completed {
                assert_eq!(j.phase, TransferPhase::Completed);
                assert_eq!(j.transferred_bytes, test_data.len() as u64);
                assert_eq!(j.total_bytes, test_data.len() as u64);
                assert!(j.checksum.is_some());
                completed = true;
                break;
            }
        }
    }

    assert!(completed, "Transfer job did not complete in time");

    // 4. Verify destination file exists and matches source
    let edit_res = EditorService::read_for_editing(&state, &admin, "local", "/dest_lifecycle.txt")
        .await
        .unwrap();
    assert_eq!(edit_res.0.len(), test_data.len());
}

#[tokio::test]
async fn test_move_cleanup_lifecycle() {
    let (state, admin, _temp) = setup_test_context().await;

    // 1. Create source file
    let test_data = b"Transactional Move Lifecycle Test".to_vec();
    FileService::create_or_write_file(
        &state,
        &admin,
        "local",
        "/move_source.txt",
        test_data.clone(),
        None,
    )
    .await
    .unwrap();

    // 2. Submit move transfer
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

    // 3. Wait for completion
    let mut completed = false;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let jobs = state
            .transfer_manager
            .list_jobs(Some(&admin.id), true, false)
            .await;
        if let Some(j) = jobs.iter().find(|j| j.id == job_id) {
            if j.status == TransferStatus::Completed {
                assert_eq!(j.phase, TransferPhase::Completed);
                completed = true;
                break;
            }
        }
    }

    assert!(completed, "Move transfer did not complete in time");

    // 4. Verify destination exists & source is removed
    let dest_res = FileService::stat_file(&state, &admin, "local", "/move_dest.txt").await;
    assert!(dest_res.is_ok());

    let src_res = FileService::stat_file(&state, &admin, "local", "/move_source.txt").await;
    assert!(src_res.is_err(), "Source file should be deleted on move");
}
