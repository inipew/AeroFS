use backend::auth::{AuthenticatedUser, UserInfo};
use backend::config::AppConfig;
use backend::db::init_db;
use backend::domain::{
    Capabilities, CommitSemantics, FileKind, FileMetadata, OperationKind, RetryPolicy,
    WriteStrategy,
};
use backend::errors::{AppError, VfsError};
use backend::services::{
    ConnectionService, CreateConnectionRequest, MetadataCache, UpdateConnectionRequest,
    UploadLockManager,
};
use backend::state::AppState;
use backend::vfs::cleanup_stale_staging_files;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;

async fn setup_test_context() -> (AppState, AuthenticatedUser, tempfile::TempDir) {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("test_plan53.db");
    let storage_dir = temp.path().join("storage");
    std::fs::create_dir_all(&storage_dir).unwrap();

    let mut config = AppConfig::default();
    config.database.url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    config.filesystem.default_local_root = storage_dir;

    let pool = init_db(&config.database.url).await.unwrap();
    let state = AppState::new_with_db(config, pool).await;

    let admin = AuthenticatedUser(UserInfo {
        id: "admin-plan53".into(),
        username: "admin".into(),
        is_admin: true,
    });

    (state, admin, temp)
}

#[tokio::test]
async fn test_in_flight_upload_lock_rejects_concurrent_same_path() {
    let lock_manager = UploadLockManager::new();

    // 1. Acquire lock on /movies/big.mkv
    let guard1 = lock_manager
        .try_acquire("conn_s3", "/movies/big.mkv")
        .await
        .unwrap();

    // 2. Second attempt for same path on same connection fails with Conflict
    let err2 = lock_manager.try_acquire("conn_s3", "/movies/big.mkv").await;
    assert!(
        err2.is_err(),
        "Concurrent upload to same path must be rejected"
    );
    match err2.unwrap_err() {
        AppError::Conflict(msg) => assert!(msg.contains("already in progress")),
        other => panic!("Expected Conflict error, got: {:?}", other),
    }

    // 3. Different connection or different path succeeds
    let _guard_diff_conn = lock_manager
        .try_acquire("conn_ftp", "/movies/big.mkv")
        .await
        .unwrap();
    let _guard_diff_path = lock_manager
        .try_acquire("conn_s3", "/movies/other.mkv")
        .await
        .unwrap();

    // 4. Dropping guard releases path lock
    drop(guard1);
    tokio::time::sleep(Duration::from_millis(50)).await;

    let guard3 = lock_manager.try_acquire("conn_s3", "/movies/big.mkv").await;
    assert!(guard3.is_ok(), "Released lock should allow new upload");
}

#[tokio::test]
async fn test_operation_aware_retry_policy() {
    let policy = RetryPolicy::new(3);

    let timeout_err = AppError::Vfs(VfsError::Timeout("connection timed out".into()));
    let checksum_err = AppError::ChecksumMismatch("mismatched sha256".into());

    // 1. Idempotent Read allows retry on timeout
    assert!(policy.is_retryable_for_operation(OperationKind::Read, &timeout_err, 1));
    assert!(policy.is_retryable_for_operation(OperationKind::Stat, &timeout_err, 1));
    assert!(policy.is_retryable_for_operation(OperationKind::List, &timeout_err, 1));

    // 2. Non-idempotent Append NEVER retries blind on timeout
    assert!(!policy.is_retryable_for_operation(OperationKind::Append, &timeout_err, 1));

    // 3. Checksum mismatch allowed on attempt 1 (rule out transient network glitch)
    assert!(policy.is_retryable_for_operation(OperationKind::Read, &checksum_err, 1));
    // Checksum mismatch rejected on attempt >= 2 (fail hard on persistent corruption)
    assert!(!policy.is_retryable_for_operation(OperationKind::Read, &checksum_err, 2));
    assert!(!policy.is_retryable_for_operation(OperationKind::Read, &checksum_err, 3));
}

#[tokio::test]
async fn test_single_flight_cache_request_coalescing() {
    let cache = MetadataCache::new(Duration::from_secs(5));
    let counter = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();
    for _ in 0..10 {
        let cache_clone = cache.clone();
        let counter_clone = Arc::clone(&counter);
        handles.push(tokio::spawn(async move {
            cache_clone
                .get_or_fetch("local", "/shared_file.txt", || async {
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    Ok(FileMetadata {
                        name: "shared_file.txt".to_string(),
                        path: "/shared_file.txt".to_string(),
                        size: 4096,
                        kind: FileKind::File,
                        modified_at: None,
                        created_at: None,
                        mime_type: None,
                        etag: "etag-123".to_string(),
                        permissions: None,
                        is_readonly: false,
                        is_hidden: false,
                        symlink_target: None,
                    })
                })
                .await
        }));
    }

    for h in handles {
        let meta = h.await.unwrap().unwrap();
        assert_eq!(meta.size, 4096);
    }

    // Coalesced: exactly 1 actual fetch execution across 10 concurrent requests
    assert_eq!(
        counter.load(Ordering::SeqCst),
        1,
        "Single-flight must coalesce 10 concurrent fetches into 1"
    );
}

#[tokio::test]
async fn test_connection_hot_swap_runtime_replacement() {
    let (state, admin, _temp) = setup_test_context().await;

    // 1. Create a connection
    let create_payload = CreateConnectionRequest {
        name: "Test FTP Server".to_string(),
        provider: backend::domain::ProviderKind::Ftp,
        host: Some("127.0.0.1".to_string()),
        port: Some(21),
        username: Some("anonymous".to_string()),
        secret: Some("secret123".to_string()),
        base_path: Some("/pub".to_string()),
        read_only: Some(false),
    };

    let conn_id = ConnectionService::create_connection(&state, &admin, create_payload)
        .await
        .unwrap();

    assert!(state.registry.get(&conn_id).await.is_some());

    // 2. Update connection properties
    let update_payload = UpdateConnectionRequest {
        name: Some("Updated FTP Server".to_string()),
        host: Some("127.0.0.1".to_string()),
        port: Some(2121),
        username: Some("user2".to_string()),
        secret: Some("newsecret456".to_string()),
        base_path: Some("/upload".to_string()),
        read_only: Some(true),
        enabled: Some(true),
    };

    let update_res =
        ConnectionService::update_connection(&state, &admin, &conn_id, update_payload).await;
    assert!(update_res.is_ok(), "Hot-swap update must succeed");

    // 3. Verify connection reflects updated name
    let detail = ConnectionService::get_connection(&state, &admin, &conn_id)
        .await
        .unwrap();
    assert_eq!(detail.connection.name, "Updated FTP Server");
    assert_eq!(detail.connection.port, Some(2121));
    assert!(detail.connection.read_only);
}

#[tokio::test]
async fn test_orphan_staging_cleanup() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("storage_root");
    std::fs::create_dir_all(&root).unwrap();

    let stale_part1 = root.join("movie.mkv.aerofs.part");
    let stale_part2 = root.join(".image.png.aerofs-part-job999");
    let valid_file = root.join("regular_file.txt");

    std::fs::write(&stale_part1, b"incomplete payload 1").unwrap();
    std::fs::write(&stale_part2, b"incomplete payload 2").unwrap();
    std::fs::write(&valid_file, b"permanent content").unwrap();

    // 1. Calling cleanup with 0s max_age purges both orphan staging files
    let cleaned = cleanup_stale_staging_files(&root, Duration::from_secs(0)).await;
    assert_eq!(cleaned, 2, "Must delete exactly 2 orphan staging files");

    assert!(!stale_part1.exists());
    assert!(!stale_part2.exists());
    assert!(
        valid_file.exists(),
        "Regular non-staging file must be preserved"
    );
}

#[tokio::test]
async fn test_write_strategy_commit_semantics_selection() {
    let caps_atomic_rename = Capabilities {
        atomic_rename: true,
        ..Default::default()
    };

    let strat1 = WriteStrategy::select(&caps_atomic_rename, true);
    assert_eq!(strat1.semantics, CommitSemantics::AtomicRename);
    assert!(strat1.safe_overwrite);

    let caps_atomic_write = Capabilities {
        atomic_rename: false,
        atomic_write: true,
        ..Default::default()
    };

    let strat2 = WriteStrategy::select(&caps_atomic_write, true);
    assert_eq!(strat2.semantics, CommitSemantics::AtomicObjectPut);
    assert!(strat2.safe_overwrite);

    let caps_direct = Capabilities {
        atomic_rename: false,
        atomic_write: false,
        ..Default::default()
    };

    let strat3 = WriteStrategy::select(&caps_direct, false);
    assert_eq!(strat3.semantics, CommitSemantics::DirectWrite);
    assert!(strat3.safe_overwrite);
}
