use backend::auth::{AuthenticatedUser, UserInfo};
use backend::bootstrap::build_application;
use backend::config::{AppConfig, ProviderStorageConfig};
use backend::db::init_db;
use backend::domain::{Actor, ChecksumCapabilities, ConnectionId, SortField, SortOrder, VfsPath};
use backend::state::{AppState, RuntimeOwner, ShutdownReason};
use backend::transfer::{
    TransferJob, TransferPhase, TransferPlanner, TransferStatus, TransferStrategy, TransferType,
};
use backend::vfs::opendal::{
    build_fs_operator, build_fs_operator_with_config, build_s3_operator, OpenDalFileSystem,
};
use backend::vfs::traits::FileSystem;
use std::sync::Arc;
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

async fn setup_test_state() -> (AppState, AuthenticatedUser, TestRuntime) {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("opendal_native_test.db");
    let storage_dir = temp.path().join("storage");
    std::fs::create_dir_all(&storage_dir).unwrap();

    let mut config = AppConfig::default();
    config.database.url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    config.filesystem.default_local_root = storage_dir;

    let db = init_db(&config.database.url).await.unwrap();
    let built = build_application(config, db).await;

    let admin = AuthenticatedUser(UserInfo {
        id: "admin-user".to_string(),
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
        id: user.id.clone(),
        username: user.username.clone(),
        is_admin: user.is_admin,
    }
}

async fn write_file(state: &AppState, user: &AuthenticatedUser, path: &str, content: Vec<u8>) {
    state
        .file_api
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

async fn list_page(
    state: &AppState,
    user: &AuthenticatedUser,
    cursor: Option<String>,
) -> backend::application::files::DirectoryListing {
    state
        .file_api
        .files
        .list_directory
        .execute(
            &actor(user),
            backend::application::files::ListDirectoryCommand {
                connection: ConnectionId::local(),
                path: Some("/".to_string()),
                show_hidden: Some(false),
                sort: Some(SortField::Name),
                order: Some(SortOrder::Asc),
                cursor,
                limit: Some(10),
            },
        )
        .await
        .unwrap()
}

fn create_test_job(
    id: &str,
    t_type: TransferType,
    src_conn: &str,
    src_path: &str,
    dst_conn: &str,
    dst_path: &str,
) -> TransferJob {
    TransferJob {
        id: id.to_string(),
        user_id: Some("test-user".to_string()),
        name: "test-transfer".to_string(),
        transfer_type: t_type,
        source_connection_id: src_conn.to_string(),
        source_path: src_path.to_string(),
        destination_connection_id: dst_conn.to_string(),
        destination_path: dst_path.to_string(),
        status: TransferStatus::Queued,
        phase: TransferPhase::Preparing,
        execution_mode: Default::default(),
        staging: Default::default(),
        transferred_bytes: 0,
        total_bytes: 1000,
        speed_bytes_per_sec: 0,
        eta_seconds: None,
        checksum: None,
        error_message: None,
        dismissed_at: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

#[tokio::test]
async fn test_opendal_streaming_lister_and_cursor_pagination() {
    let (state, admin, _runtime) = setup_test_state().await;

    for i in 0..25 {
        let path = format!("/page_item_{:02}.txt", i);
        let content = format!("Content {}", i).into_bytes();
        write_file(&state, &admin, &path, content).await;
    }

    let page1 = list_page(&state, &admin, None).await;
    assert_eq!(page1.entries.len(), 10);
    assert!(page1.next_cursor.is_some());

    let page2 = list_page(&state, &admin, page1.next_cursor.clone()).await;
    assert_eq!(page2.entries.len(), 10);
    assert!(page2.next_cursor.is_some());
    assert_ne!(page1.entries[0].name, page2.entries[0].name);

    let page3 = list_page(&state, &admin, page2.next_cursor.clone()).await;
    assert_eq!(page3.entries.len(), 5);
    assert!(page3.next_cursor.is_none());
}

#[tokio::test]
async fn test_opendal_transfer_planner_strategy_selection() {
    let temp = tempdir().unwrap();
    let root_str = temp.path().to_string_lossy().to_string();
    let op = build_fs_operator(&root_str).unwrap();
    let local_fs: Arc<dyn FileSystem> = Arc::new(OpenDalFileSystem::new("local_conn", op));

    let s3_op = build_s3_operator("my-bucket", None, None, None, None, None).unwrap();
    let s3_fs: Arc<dyn FileSystem> = Arc::new(OpenDalFileSystem::new("s3_conn", s3_op));

    let src_vfs = VfsPath::new("local_conn", "/file_a.txt").unwrap();
    let dst_vfs = VfsPath::new("local_conn", "/file_b.txt").unwrap();

    let move_job = create_test_job("job-1", TransferType::Move, "local_conn", "/file_a.txt", "local_conn", "/file_b.txt");
    let strategy_move = TransferPlanner::plan_transfer(&move_job, &local_fs, &local_fs, &src_vfs, &dst_vfs);
    assert_eq!(strategy_move, TransferStrategy::NativeRename);

    let cross_job = create_test_job("job-2", TransferType::Copy, "local_conn", "/file_a.txt", "s3_conn", "/file_b.txt");
    let cross_dst = VfsPath::new("s3_conn", "/file_b.txt").unwrap();
    let strategy_cross = TransferPlanner::plan_transfer(&cross_job, &local_fs, &s3_fs, &src_vfs, &cross_dst);
    assert_eq!(strategy_cross, TransferStrategy::Streaming);

    let s3_copy_job = create_test_job("job-3", TransferType::Copy, "s3_conn", "/file_a.txt", "s3_conn", "/file_b.txt");
    let s3_src = VfsPath::new("s3_conn", "/file_a.txt").unwrap();
    let strategy_s3_copy = TransferPlanner::plan_transfer(&s3_copy_job, &s3_fs, &s3_fs, &s3_src, &cross_dst);
    assert_eq!(strategy_s3_copy, TransferStrategy::ServerSideCopy);
}

#[tokio::test]
async fn test_opendal_presign_support_trait_and_rejection_policy() {
    let temp = tempdir().unwrap();
    let root_str = temp.path().to_string_lossy().to_string();
    let op = build_fs_operator(&root_str).unwrap();
    let local_fs = OpenDalFileSystem::new("local_conn", op);
    assert!(local_fs.as_presign().is_none());

    let s3_op = build_s3_operator("bucket-test", Some("us-east-1"), None, Some("ak"), Some("sk"), None).unwrap();
    let s3_fs = OpenDalFileSystem::new("s3_conn", s3_op);
    assert!(s3_fs.as_presign().is_some());
    let presign = s3_fs.as_presign().unwrap();

    let s3_vfs = VfsPath::new("s3_conn", "/data.bin").unwrap();
    let presign_read_res = presign.presign_read_url(&s3_vfs, std::time::Duration::from_secs(300)).await;
    assert!(presign_read_res.is_ok());
    let url = presign_read_res.unwrap();
    assert!(url.contains("bucket-test") || url.contains("data.bin") || url.contains("X-Amz"));
}

#[tokio::test]
async fn test_opendal_storage_config_and_common_layers() {
    let custom_cfg = ProviderStorageConfig {
        max_concurrency: 32,
        control_timeout_secs: 12,
        io_timeout_secs: 90,
        retry_attempts: 2,
    };

    let temp = tempdir().unwrap();
    let root_str = temp.path().to_string_lossy().to_string();
    let fs_op = build_fs_operator_with_config(&root_str, Some(&custom_cfg)).unwrap();
    let vfs = OpenDalFileSystem::new("local_layered", fs_op);

    let test_path = VfsPath::new("local_layered", "/hello.txt").unwrap();
    vfs.create_file(&test_path).await.unwrap();
    assert!(vfs.stat(&test_path).await.is_ok());
}

#[tokio::test]
async fn test_opendal_checksum_capabilities_granularity() {
    let s3_caps = ChecksumCapabilities::s3_default();
    assert!(s3_caps.md5);
    assert!(s3_caps.crc32);
    assert!(s3_caps.crc32c);
    assert!(s3_caps.sha1);
    assert!(s3_caps.sha256);

    let all_caps = ChecksumCapabilities::all();
    assert!(all_caps.sha1);
    assert!(all_caps.sha256);
    assert!(all_caps.has_any());

    let none_caps = ChecksumCapabilities::default();
    assert!(!none_caps.has_any());
}
