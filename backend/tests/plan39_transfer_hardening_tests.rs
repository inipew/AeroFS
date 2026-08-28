use backend::auth::{AuthenticatedUser, UserInfo};
use backend::config::AppConfig;
use backend::db::init_db;
use backend::domain::ProviderKind;
use backend::services::{
    ConnectionService, CreateConnectionRequest, EditorService, FileService, TransferService,
};
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

#[tokio::test]
async fn test_realtime_cancellation_with_token() {
    let (state, admin, _temp) = setup_test_context().await;

    // 1. Create large source file (32 MB) on disk to ensure in-flight cancellation window
    let test_data = vec![b'X'; 32 * 1024 * 1024];
    std::fs::write(
        _temp.path().join("storage").join("source_cancel_test.dat"),
        &test_data,
    )
    .unwrap();

    // 2. Submit transfer
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

    // 3. Immediately request cancellation
    let cancel_res = TransferService::cancel_transfer(&state, &admin, &job_id).await;
    assert!(cancel_res.is_ok());

    // 4. Wait for cancellation to settle
    let mut cancelled = false;
    for _ in 0..80 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let jobs = state
            .transfer_manager
            .list_jobs(Some(&admin.id), true, true)
            .await;
        if let Some(j) = jobs.iter().find(|j| j.id == job_id) {
            if j.status == TransferStatus::Cancelled {
                cancelled = true;
                break;
            }
        }
    }

    assert!(cancelled, "Transfer job should transition to Cancelled");

    // 5. Verify staging hidden .aerofs-part file is cleaned up
    let part_path = format!("/.dest_cancel_test.dat.aerofs-part-{}", job_id);
    let part_stat = FileService::stat_file(&state, &admin, "local", &part_path).await;
    assert!(
        part_stat.is_err(),
        "Staging part file should be deleted on cancellation"
    );
}

#[tokio::test]
async fn test_directory_transfer_bounded_limits_and_creation() {
    let (state, admin, _temp) = setup_test_context().await;

    // 1. Create directory tree with files
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

    // 2. Submit directory copy transfer
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

    assert!(completed, "Directory transfer did not complete in time");

    // 4. Verify destination directory and files exist
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

    // 1. Create a dummy secondary local connection
    let remote_dir = temp.path().join("dummy_remote");
    std::fs::create_dir_all(&remote_dir).unwrap();

    let conn_id = ConnectionService::create_connection(
        &state,
        &admin,
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

    // 2. Create large file in local
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

    // 3. Submit transfer to dummy_remote
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

    // 4. Delete the connection (should cancel all queued/active transfers for this connection)
    ConnectionService::delete_connection(&state, &admin, &conn_id)
        .await
        .unwrap();

    // 5. Verify the transfer job was cancelled or aborted
    let mut settled = false;
    let mut last_status = None;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let jobs = state
            .transfer_manager
            .list_jobs(Some(&admin.id), true, true)
            .await;
        if let Some(j) = jobs.iter().find(|j| j.id == job_id) {
            last_status = Some(j.status);
            if j.status == TransferStatus::Cancelled || j.status == TransferStatus::Failed {
                settled = true;
                break;
            }
        }
    }

    assert!(
        settled,
        "Transfer should be cancelled or aborted upon connection deletion, actual status: {:?}",
        last_status
    );
}
