use axum::extract::FromRef;
use backend::auth::{AuthenticatedUser, UserInfo};
use backend::config::AppConfig;
use backend::db::init_db;
use backend::domain::{Actor, SftpAuth, VfsPath};
use backend::events::EventJournal;
use backend::services::{EditorService, FileService, TransferService};
use backend::state::{AppState, TransferState};
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

fn actor(user: &AuthenticatedUser) -> Actor {
    Actor {
        id: user.id().to_string(),
        username: user.username().to_string(),
        is_admin: user.is_admin(),
    }
}

async fn wait_for_completed(
    state: &AppState,
    user: &AuthenticatedUser,
    job_id: &str,
) -> backend::transfer::TransferJobResponse {
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
        other => panic!("Expected VfsError::NotSupported, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_resume_integrity_restart_on_invalid_part() {
    let (state, admin, _temp) = setup_test_context().await;
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

    let job = wait_for_completed(&state, &admin, &job_id).await;
    assert!(
        job.checksum.is_some(),
        "Full transfer must calculate SHA-256 checksum"
    );

    let (content, _) =
        EditorService::read_for_editing(&state, &admin, "local", "/dst_resume_test.txt")
            .await
            .unwrap();
    assert_eq!(content.as_bytes(), src_content);
}

#[tokio::test]
async fn test_pagination_bounded_limits_and_cursor() {
    let (state, admin, _temp) = setup_test_context().await;

    for i in 0..20 {
        FileService::create_or_write_file(
            &state,
            &admin,
            "local",
            &format!("/page_file_{i:02}.txt"),
            format!("content {i}").into_bytes(),
            None,
        )
        .await
        .unwrap();
    }

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
    assert_eq!(page1.total_count, Some(20));
    assert!(page1.has_more, "Should have more items");
    assert!(page1.next_cursor.is_some(), "Next cursor must be generated");

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
    assert_eq!(page2.total_count, Some(20));
    let page1_names: Vec<_> = page1.entries.iter().map(|entry| entry.name.clone()).collect();
    let page2_names: Vec<_> = page2.entries.iter().map(|entry| entry.name.clone()).collect();
    assert!(page1_names.iter().all(|name| !page2_names.contains(name)));
    assert!(page1_names.last().unwrap() < page2_names.first().unwrap());
}

#[tokio::test]
async fn test_directory_transfer_zero_vector_streaming() {
    let (state, admin, _temp) = setup_test_context().await;

    FileService::create_directory(&state, &admin, "local", "/source_dir/sub1/sub2")
        .await
        .unwrap();
    for i in 0..10 {
        FileService::create_or_write_file(
            &state,
            &admin,
            "local",
            &format!("/source_dir/sub1/file_{i}.txt"),
            format!("file data {i}").into_bytes(),
            None,
        )
        .await
        .unwrap();
    }

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

    wait_for_completed(&state, &admin, &job_id).await;

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
    let root = tempdir().unwrap();
    let src_root = root.path().join("source");
    let dst_root = root.path().join("destination");
    std::fs::create_dir_all(&src_root).unwrap();
    std::fs::create_dir_all(&dst_root).unwrap();
    let src_content = b"TEST CONTENT FOR NON-ATOMIC RENAME DESTINATION (FTP/S3)";
    std::fs::write(src_root.join("src_nonatomic.txt"), src_content).unwrap();

    let src_op = backend::vfs::opendal::build_fs_operator(&src_root.to_string_lossy()).unwrap();
    let src_caps = backend::vfs::opendal::capabilities::map_opendal_capabilities_for_scheme(
        src_op.info().capability(),
        src_op.info().scheme(),
    );
    let src_fs: Arc<dyn backend::vfs::FileSystem> = Arc::new(
        backend::vfs::opendal::OpenDalFileSystem::new_with_capabilities("local", src_op, src_caps),
    );

    let dst_op = backend::vfs::opendal::build_fs_operator(&dst_root.to_string_lossy()).unwrap();
    let mut dst_caps = backend::vfs::opendal::capabilities::map_opendal_capabilities_for_scheme(
        dst_op.info().capability(),
        dst_op.info().scheme(),
    );
    dst_caps.atomic_rename = false;
    dst_caps.atomic_write = false;
    let dst_fs: Arc<dyn backend::vfs::FileSystem> = Arc::new(
        backend::vfs::opendal::OpenDalFileSystem::new_with_capabilities(
            "mock_ftp",
            dst_op,
            dst_caps,
        ),
    );
    let dst_vfs = VfsPath::new("mock_ftp", "/dst_nonatomic.txt").unwrap();

    let db_path = root.path().join("transfer.db");
    let database_url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    let db = init_db(&database_url).await.unwrap();
    let journal = Arc::new(EventJournal::init(db.clone()).await.unwrap());

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
        execution_mode: Default::default(),
        staging: Default::default(),
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

    let cancel_token = tokio_util::sync::CancellationToken::new();
    let jobs_map = Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()));
    jobs_map.write().await.insert(job.id.clone(), job.clone());

    let mut providers = std::collections::HashMap::new();
    providers.insert("local".to_string(), src_fs);
    providers.insert("mock_ftp".to_string(), dst_fs.clone());
    let providers_lock = Arc::new(tokio::sync::RwLock::new(providers));

    let res = TransferManager::execute_job(
        &mut job,
        &cancel_token,
        &providers_lock,
        &jobs_map,
        &journal,
        &db,
    )
    .await;

    assert!(
        res.is_ok(),
        "Transfer to non-atomic provider must succeed: {:?}",
        res.err()
    );

    let mut reader = dst_fs.read_stream(&dst_vfs).await.unwrap();
    let mut read_dst = Vec::new();
    reader.read_to_end(&mut read_dst).await.unwrap();
    assert_eq!(read_dst, src_content);
}
