use backend::auth::{AuthenticatedUser, UserInfo};
use backend::config::AppConfig;
use backend::db::init_db;
use backend::domain::{parse_single_byte_range, Capabilities, RangeError};
use backend::errors::AppError;
use backend::services::{FileService, TransferService};
use backend::transfer::{TransferJob, TransferPhase, TransferStatus, TransferType};
use backend::AppState;
use chrono::Utc;
use std::collections::HashSet;
use tempfile::tempdir;

async fn setup_test_context() -> (
    AppState,
    AuthenticatedUser,
    AuthenticatedUser,
    tempfile::TempDir,
) {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("plan37_test.db");
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

    let regular = AuthenticatedUser(UserInfo {
        id: "regular-id".to_string(),
        username: "regular".to_string(),
        is_admin: false,
    });

    (state, admin, regular, temp)
}

#[tokio::test]
async fn test_if_match_strict_preconditions() {
    let (state, admin, _regular, _temp) = setup_test_context().await;

    // 1. Initial write
    let initial_content = b"Initial content for ETag test".to_vec();
    let meta1 = FileService::create_or_write_file(
        &state,
        &admin,
        "local",
        "/test_etag.txt",
        initial_content,
        None,
    )
    .await
    .expect("Initial write should succeed");

    assert!(!meta1.etag.is_empty());

    // 2. Write with valid matching If-Match ETag -> Must succeed
    let updated_content = b"Updated content with valid ETag".to_vec();
    let meta2 = FileService::create_or_write_file(
        &state,
        &admin,
        "local",
        "/test_etag.txt",
        updated_content,
        Some(&meta1.etag),
    )
    .await
    .expect("Write with matching ETag should succeed");

    assert_ne!(meta1.etag, meta2.etag);

    // 3. Write with stale/mismatching If-Match ETag -> Must fail with PreconditionFailed
    let stale_write = FileService::create_or_write_file(
        &state,
        &admin,
        "local",
        "/test_etag.txt",
        b"Conflicting write".to_vec(),
        Some(&meta1.etag), // Using old stale ETag
    )
    .await;

    assert!(
        matches!(stale_write, Err(AppError::PreconditionFailed(_))),
        "Expected PreconditionFailed for stale ETag, got: {:?}",
        stale_write
    );

    // 4. Write with If-Match on non-existent file -> Must fail (never silently bypass)
    let non_existent_write = FileService::create_or_write_file(
        &state,
        &admin,
        "local",
        "/does_not_exist.txt",
        b"New file with expected etag".to_vec(),
        Some("\"some-etag\""),
    )
    .await;

    assert!(
        matches!(non_existent_write, Err(AppError::PreconditionFailed(_))),
        "Expected PreconditionFailed when target file does not exist for If-Match, got: {:?}",
        non_existent_write
    );
}

#[test]
fn test_rfc_range_parser_comprehensive() {
    let file_size = 1000u64;

    // Bounded range
    let r1 = parse_single_byte_range("bytes=0-499", file_size).unwrap();
    assert_eq!(r1.start, 0);
    assert_eq!(r1.end, 499);
    assert_eq!(r1.length(), 500);
    assert_eq!(r1.content_range_header(), "bytes 0-499/1000");

    // Open-ended range
    let r2 = parse_single_byte_range("bytes=500-", file_size).unwrap();
    assert_eq!(r2.start, 500);
    assert_eq!(r2.end, 999);
    assert_eq!(r2.length(), 500);

    // Suffix range
    let r3 = parse_single_byte_range("bytes=-300", file_size).unwrap();
    assert_eq!(r3.start, 700);
    assert_eq!(r3.end, 999);
    assert_eq!(r3.length(), 300);

    // Multi-range rejection
    assert_eq!(
        parse_single_byte_range("bytes=0-100,200-300", file_size),
        Err(RangeError::MultiRangeNotSupported)
    );

    // Out of bounds / unsatisfiable
    assert_eq!(
        parse_single_byte_range("bytes=1500-", file_size),
        Err(RangeError::NotSatisfiable(1000))
    );

    // Zero-length file
    assert_eq!(
        parse_single_byte_range("bytes=0-0", 0),
        Err(RangeError::NotSatisfiable(0))
    );
}

#[tokio::test]
async fn test_batch_delete_deduplication_and_nesting() {
    let (state, admin, _regular, _temp) = setup_test_context().await;

    // Setup nested files & directory
    FileService::create_directory(&state, &admin, "local", "/batch_test/nested")
        .await
        .unwrap();
    FileService::create_or_write_file(
        &state,
        &admin,
        "local",
        "/batch_test/nested/file1.txt",
        b"1".to_vec(),
        None,
    )
    .await
    .unwrap();
    FileService::create_or_write_file(
        &state,
        &admin,
        "local",
        "/batch_test/file2.txt",
        b"2".to_vec(),
        None,
    )
    .await
    .unwrap();

    // Call delete_files with duplicates and mixed nested paths
    let paths_to_delete = vec![
        "/batch_test/nested/file1.txt".to_string(),
        "/batch_test/nested/file1.txt".to_string(), // Duplicate
        "/batch_test/nested".to_string(),
        "/batch_test/file2.txt".to_string(),
        "/batch_test".to_string(),
        "/batch_test".to_string(), // Duplicate
    ];

    let (succeeded, failed) = FileService::delete_files(&state, &admin, "local", paths_to_delete)
        .await
        .expect("Batch delete should execute");

    assert!(
        failed.is_empty(),
        "Expected zero failures, got: {:?}",
        failed
    );
    assert!(!succeeded.is_empty());
}

#[tokio::test]
async fn test_transfer_visibility_and_interrupted_status() {
    let admin = AuthenticatedUser(UserInfo {
        id: "admin-user".into(),
        username: "admin".into(),
        is_admin: true,
    });
    let owner = AuthenticatedUser(UserInfo {
        id: "owner-user".into(),
        username: "owner".into(),
        is_admin: false,
    });
    let stranger = AuthenticatedUser(UserInfo {
        id: "stranger-user".into(),
        username: "stranger".into(),
        is_admin: false,
    });

    let now = Utc::now();
    let job = TransferJob {
        id: "job-123".into(),
        user_id: Some("owner-user".into()),
        name: "test-transfer".into(),
        transfer_type: TransferType::Copy,
        status: TransferStatus::Interrupted,
        phase: TransferPhase::Finalizing,
        execution_mode: Default::default(),
        staging: Default::default(),
        source_connection_id: "local".into(),
        source_path: "/source".into(),
        destination_connection_id: "remote_s3".into(),
        destination_path: "/dest".into(),
        total_bytes: 1000,
        transferred_bytes: 500,
        speed_bytes_per_sec: 0,
        eta_seconds: None,
        checksum: None,
        error_message: Some("Transfer interrupted by server restart".into()),
        created_at: now,
        updated_at: now,
        dismissed_at: None,
    };

    // 1. Status string roundtrip
    assert_eq!(TransferStatus::Interrupted.as_str(), "interrupted");
    assert_eq!(
        TransferStatus::from_str("interrupted"),
        TransferStatus::Interrupted
    );

    // 2. Visibility: Admin sees job
    let mut allowed_empty = HashSet::new();
    assert!(TransferService::authorize_transfer_visibility(
        &admin,
        &job,
        &allowed_empty
    ));

    // 3. Visibility: Owner sees their own job even with no third-party permissions in set
    assert!(TransferService::authorize_transfer_visibility(
        &owner,
        &job,
        &allowed_empty
    ));

    // 4. Visibility: Stranger without permissions cannot see job
    assert!(!TransferService::authorize_transfer_visibility(
        &stranger,
        &job,
        &allowed_empty
    ));

    // 5. Visibility: Stranger with permissions to both connections CAN see job
    allowed_empty.insert("local".into());
    allowed_empty.insert("remote_s3".into());
    assert!(TransferService::authorize_transfer_visibility(
        &stranger,
        &job,
        &allowed_empty
    ));
}

#[tokio::test]
async fn test_provider_capabilities_verification() {
    let local_caps = Capabilities::local_default();
    assert!(local_caps.atomic_write);
    assert!(local_caps.atomic_rename);
    assert!(local_caps.permissions);
    assert!(local_caps.range_read);
}
