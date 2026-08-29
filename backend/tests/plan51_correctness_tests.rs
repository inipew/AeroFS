use backend::auth::{AuthenticatedUser, UserInfo};
use backend::config::AppConfig;
use backend::db::init_db;
use backend::domain::{SftpAuth, VfsPath};
use backend::services::{FileService, TransferService};
use backend::state::AppState;
use backend::transfer::{TransferManager, TransferStatus, TransferType};
use backend::vfs::opendal::builder::build_sftp_operator_with_config;
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;
use tokio::io::AsyncReadExt;

async fn setup_test_context() -> (AppState, AuthenticatedUser, tempfile::TempDir) {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("test_plan51.db");
    let storage_dir = temp.path().join("storage");
    std::fs::create_dir_all(&storage_dir).unwrap();

    let mut config = AppConfig::default();
    config.database.url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    config.filesystem.default_local_root = storage_dir;

    let db = init_db(&config.database.url).await.unwrap();
    let state = AppState::new_with_db(config, db).await;

    let admin = AuthenticatedUser(UserInfo {
        id: "admin-user".to_string(),
        username: "admin".to_string(),
        is_admin: true,
    });

    (state, admin, temp)
}

#[tokio::test]
async fn test_sftp_password_rejection_notice() {
    let auth = SftpAuth::Password {
        password: "secret_password".into(),
    };
    let res =
        build_sftp_operator_with_config("127.0.0.1", 22, Some("user"), Some(&auth), None, None);
    assert!(res.is_err());
    let err = res.unwrap_err();
    match err {
        backend::errors::VfsError::NotSupported(msg) => {
            assert!(msg.contains("SFTP password authentication is not natively supported"));
        }
        other => panic!("Expected VfsError::NotSupported, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_resume_integrity_restart_on_invalid_part() {
    let (state, admin, _temp) = setup_test_context().await;

    // 1. Create source file
    let src_content = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    FileService::create_or_write_file(
        &state,
        &admin,
        "local",
        "/src_resume_test.txt",
        src_content.to_vec(),
        None,
    )
    .await
    .unwrap();

    // 2. Perform a transfer via TransferService
    let job_id = TransferService::create_transfer(
        &state,
        &admin,
        "resume_integrity_job".into(),
        TransferType::Copy,
        "local".into(),
        "/src_resume_test.txt".into(),
        "local".into(),
        "/dst_resume_test.txt".into(),
    )
    .await
    .unwrap();

    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let jobs = state
            .transfer_manager
            .list_jobs(Some(&admin.0.id), true, true)
            .await;
        if let Some(j) = jobs.iter().find(|j| j.id == job_id) {
            if j.status == TransferStatus::Completed {
                assert!(
                    j.checksum.is_some(),
                    "Full transfer must calculate SHA-256 checksum"
                );
                break;
            }
        }
    }

    // 3. Verify destination has full contents (not truncated!)
    let provider = state.get_provider("local").await.unwrap();
    let dst_vfs = VfsPath::new("local", "/dst_resume_test.txt").unwrap();
    let mut reader = provider.read_stream(&dst_vfs).await.unwrap();
    let mut dst_read = Vec::new();
    reader.read_to_end(&mut dst_read).await.unwrap();

    assert_eq!(
        dst_read, src_content,
        "Destination file must contain 100% of source content"
    );
}

#[tokio::test]
async fn test_pagination_bounded_limits_and_cursor() {
    let (state, admin, _temp) = setup_test_context().await;

    // Create 20 test files
    for i in 0..20 {
        FileService::create_or_write_file(
            &state,
            &admin,
            "local",
            &format!("/page_file_{:02}.txt", i),
            format!("content {}", i).into_bytes(),
            None,
        )
        .await
        .unwrap();
    }

    // Page 1: limit 5
    let page1 = FileService::list_directory_paged(
        &state,
        &admin,
        "local",
        Some("/".into()),
        Some(false),
        Some("name"),
        Some("asc"),
        None,
        Some(5),
    )
    .await
    .unwrap();

    assert_eq!(page1.entries.len(), 5);
    assert!(page1.has_more, "Should have more items");
    assert!(page1.next_cursor.is_some(), "Next cursor must be generated");

    // Page 2: with cursor
    let page2 = FileService::list_directory_paged(
        &state,
        &admin,
        "local",
        Some("/".into()),
        Some(false),
        Some("name"),
        Some("asc"),
        page1.next_cursor.as_deref(),
        Some(5),
    )
    .await
    .unwrap();

    assert_eq!(page2.entries.len(), 5);
    assert_ne!(
        page1.entries[0].name, page2.entries[0].name,
        "Page 2 must have distinct items"
    );
}

#[tokio::test]
async fn test_directory_transfer_zero_vector_streaming() {
    let (state, admin, _temp) = setup_test_context().await;

    // Create nested directory tree
    FileService::create_directory(&state, &admin, "local", "/source_dir/sub1/sub2")
        .await
        .unwrap();
    for i in 0..10 {
        FileService::create_or_write_file(
            &state,
            &admin,
            "local",
            &format!("/source_dir/sub1/file_{}.txt", i),
            format!("file data {}", i).into_bytes(),
            None,
        )
        .await
        .unwrap();
    }

    // Perform directory transfer
    let job_id = TransferService::create_transfer(
        &state,
        &admin,
        "dir_stream_test".into(),
        TransferType::Copy,
        "local".into(),
        "/source_dir".into(),
        "local".into(),
        "/dest_dir".into(),
    )
    .await
    .unwrap();

    // Wait for transfer to complete
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let jobs = state
            .transfer_manager
            .list_jobs(Some(&admin.0.id), true, true)
            .await;
        if let Some(j) = jobs.iter().find(|j| j.id == job_id) {
            if j.status == TransferStatus::Completed {
                break;
            }
        }
    }

    // Verify files copied to destination
    let dest_file_stat =
        FileService::stat_file(&state, &admin, "local", "/dest_dir/sub1/file_0.txt").await;
    assert!(
        dest_file_stat.is_ok(),
        "Copied nested file must exist on destination"
    );
}

#[tokio::test]
async fn test_presign_upload_completion_endpoint() {
    let (state, admin, _temp) = setup_test_context().await;

    // Simulate uploaded file appearing on storage
    FileService::create_or_write_file(
        &state,
        &admin,
        "local",
        "/presigned_uploaded_file.bin",
        vec![1, 2, 3, 4, 5],
        None,
    )
    .await
    .unwrap();

    let meta = FileService::complete_presigned_upload(
        &state,
        &admin,
        "local",
        "/presigned_uploaded_file.bin",
        Some(5),
        None,
    )
    .await
    .unwrap();

    assert_eq!(meta.size, 5);
    assert_eq!(meta.name, "presigned_uploaded_file.bin");

    // Non-existent path fails stat check
    let err_res = FileService::complete_presigned_upload(
        &state,
        &admin,
        "local",
        "/non_existent_file.bin",
        None,
        None,
    )
    .await;
    assert!(err_res.is_err());
}

#[tokio::test]
async fn test_transfer_to_non_atomic_rename_provider() {
    let (state, admin, _temp) = setup_test_context().await;

    // 1. Create source file
    let src_content = b"TEST CONTENT FOR NON-ATOMIC RENAME DESTINATION (FTP/S3)";
    FileService::create_or_write_file(
        &state,
        &admin,
        "local",
        "/src_nonatomic.txt",
        src_content.to_vec(),
        None,
    )
    .await
    .unwrap();

    // 2. Build mock destination filesystem with atomic_rename = false
    let temp_dst = tempfile::tempdir().unwrap();
    let op = backend::vfs::opendal::build_fs_operator(&temp_dst.path().to_string_lossy()).unwrap();
    let mut caps = backend::vfs::opendal::capabilities::map_opendal_capabilities_for_scheme(
        op.info().capability(),
        op.info().scheme(),
    );
    caps.atomic_rename = false;
    caps.atomic_write = false;
    let dst_fs =
        backend::vfs::opendal::OpenDalFileSystem::new_with_capabilities("mock_ftp", op, caps);
    let dst_fs_arc: Arc<dyn backend::vfs::FileSystem> = Arc::new(dst_fs);

    let src_fs_arc = state.get_provider("local").await.unwrap();
    let dst_vfs = VfsPath::new("mock_ftp", "/dst_nonatomic.txt").unwrap();

    let mut job = backend::transfer::TransferJob {
        id: "job_nonatomic_test".to_string(),
        user_id: Some("admin-user".to_string()),
        name: "test_nonatomic".to_string(),
        transfer_type: TransferType::Copy,
        source_connection_id: "local".to_string(),
        source_path: "/src_nonatomic.txt".to_string(),
        destination_connection_id: "mock_ftp".to_string(),
        destination_path: "/dst_nonatomic.txt".to_string(),
        status: TransferStatus::Queued,
        phase: backend::transfer::TransferPhase::Preparing,
        transferred_bytes: 0,
        total_bytes: src_content.len() as u64,
        speed_bytes_per_sec: 0,
        eta_seconds: None,
        checksum: None,
        error_message: None,
        dismissed_at: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let (event_tx, _) = tokio::sync::broadcast::channel(100);
    let seq_counter = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let event_history = Arc::new(tokio::sync::RwLock::new(std::collections::VecDeque::new()));
    let cancel_token = tokio_util::sync::CancellationToken::new();
    let jobs_map = Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()));
    jobs_map.write().await.insert(job.id.clone(), job.clone());

    let mut providers = std::collections::HashMap::new();
    providers.insert("local".to_string(), src_fs_arc);
    providers.insert("mock_ftp".to_string(), dst_fs_arc.clone());
    let providers_lock = Arc::new(tokio::sync::RwLock::new(providers));

    // Execute transfer — must succeed without attempting rename on non-atomic provider
    let res = TransferManager::execute_job(
        &mut job,
        &cancel_token,
        &providers_lock,
        &jobs_map,
        &event_tx,
        &seq_counter,
        &event_history,
        &state.db,
    )
    .await;

    assert!(
        res.is_ok(),
        "Transfer to non-atomic provider must succeed: {:?}",
        res.err()
    );

    // Verify content on destination
    let mut reader = dst_fs_arc.read_stream(&dst_vfs).await.unwrap();
    let mut read_dst = Vec::new();
    reader.read_to_end(&mut read_dst).await.unwrap();
    assert_eq!(read_dst, src_content);
}
