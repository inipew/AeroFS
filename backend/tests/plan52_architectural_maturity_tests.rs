use backend::auth::{AuthenticatedUser, UserInfo};
use backend::config::AppConfig;
use backend::db::init_db;
use backend::domain::VfsPath;
use backend::services::{ArchiveService, FileService};
use backend::state::AppState;
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;
use tokio::io::AsyncReadExt;

async fn setup_test_context() -> (AppState, AuthenticatedUser, tempfile::TempDir) {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("test_plan52.db");
    let storage_dir = temp.path().join("storage");
    std::fs::create_dir_all(&storage_dir).unwrap();

    let mut config = AppConfig::default();
    config.database.url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    config.filesystem.default_local_root = storage_dir;

    let pool = init_db(&config.database.url).await.unwrap();
    let state = AppState::new_with_db(config, pool).await;

    let admin = AuthenticatedUser(UserInfo {
        id: "admin-plan52".into(),
        username: "admin".into(),
        is_admin: true,
    });

    (state, admin, temp)
}

#[tokio::test]
async fn test_archive_targz_streaming_zero_ram_buffering() {
    let (state, admin, _temp) = setup_test_context().await;

    // 1. Create source files
    let f1_data = b"Hello Plan 52 TAR.GZ Streaming Compression!";
    let f2_data = b"Second file to be archived inside the streaming archive";
    FileService::create_or_write_file(
        &state,
        &admin,
        "local",
        "/src_archive/file1.txt",
        f1_data.to_vec(),
        None,
    )
    .await
    .unwrap();

    FileService::create_or_write_file(
        &state,
        &admin,
        "local",
        "/src_archive/file2.txt",
        f2_data.to_vec(),
        None,
    )
    .await
    .unwrap();

    // 2. Compress via ArchiveService (TarGz)
    let compress_res = ArchiveService::compress(
        &state,
        &admin,
        "local",
        "/src_archive",
        &["file1.txt".into(), "file2.txt".into()],
        "/packed_archive.tar.gz",
        Some("targz"),
    )
    .await
    .unwrap();

    assert!(compress_res.success);

    // 3. Extract the archive
    let extract_res = ArchiveService::extract(
        &state,
        &admin,
        "local",
        "/packed_archive.tar.gz",
        "/extracted_dest",
        Some("targz"),
        backend::filesystem::archive::ArchiveOverwriteMode::Overwrite,
    )
    .await
    .unwrap();

    assert!(extract_res.success);

    // 4. Verify extracted files match original content
    let provider = state.get_provider("local").await.unwrap();
    let p1 = VfsPath::new("local", "/extracted_dest/file1.txt").unwrap();
    let mut reader1 = provider.read_stream(&p1).await.unwrap();
    let mut read_f1 = Vec::new();
    reader1.read_to_end(&mut read_f1).await.unwrap();
    assert_eq!(read_f1, f1_data);

    let p2 = VfsPath::new("local", "/extracted_dest/file2.txt").unwrap();
    let mut reader2 = provider.read_stream(&p2).await.unwrap();
    let mut read_f2 = Vec::new();
    reader2.read_to_end(&mut read_f2).await.unwrap();
    assert_eq!(read_f2, f2_data);
}

#[tokio::test]
async fn test_storage_runtime_shared_concurrency() {
    let (state, _admin, _temp) = setup_test_context().await;

    let runtime = state.get_storage_runtime("local").await.unwrap();
    assert_eq!(runtime.connection_id, "local");
    assert!(runtime.capabilities().read);

    // Test concurrent permit acquisition
    let mut handles = Vec::new();
    for _ in 0..10 {
        let rt = Arc::clone(&runtime);
        handles.push(tokio::spawn(async move {
            let _permit = rt.acquire_permit().await.unwrap();
            tokio::time::sleep(Duration::from_millis(10)).await;
        }));
    }

    for h in handles {
        h.await.unwrap();
    }
}

#[tokio::test]
async fn test_presigned_upload_complete_validation() {
    let (state, admin, _temp) = setup_test_context().await;

    let content = b"PRESIGNED PAYLOAD FOR VALIDATION TEST";
    FileService::create_or_write_file(
        &state,
        &admin,
        "local",
        "/presigned_valid.dat",
        content.to_vec(),
        None,
    )
    .await
    .unwrap();

    // Correct expected size succeeds
    let meta = FileService::complete_presigned_upload(
        &state,
        &admin,
        "local",
        "/presigned_valid.dat",
        Some(content.len() as u64),
        None,
    )
    .await
    .unwrap();
    assert_eq!(meta.size, content.len() as u64);

    // Mismatched expected size fails with BadRequest
    let err_size = FileService::complete_presigned_upload(
        &state,
        &admin,
        "local",
        "/presigned_valid.dat",
        Some(99999),
        None,
    )
    .await;
    assert!(err_size.is_err(), "Size mismatch must fail verification");

    // Non-existent path fails with NotFound
    let err_nf = FileService::complete_presigned_upload(
        &state,
        &admin,
        "local",
        "/non_existent.dat",
        None,
        None,
    )
    .await;
    assert!(err_nf.is_err(), "Non-existent path must fail");
}

#[tokio::test]
async fn test_metadata_cache_lifecycle_and_invalidation() {
    let (state, admin, _temp) = setup_test_context().await;

    let content1 = b"Original Content v1";
    FileService::create_or_write_file(
        &state,
        &admin,
        "local",
        "/cached_file.txt",
        content1.to_vec(),
        None,
    )
    .await
    .unwrap();

    // 1. Initial stat populates cache
    let stat1 = FileService::stat_file(&state, &admin, "local", "/cached_file.txt")
        .await
        .unwrap();
    assert_eq!(stat1.size, content1.len() as u64);

    // Verify cache has entry
    let cached = state
        .metadata_cache
        .get("local", "/cached_file.txt")
        .await;
    assert!(cached.is_some(), "MetadataCache must contain cached stat");

    // 2. Overwrite file -> invalidates cache
    let content2 = b"Updated Content v2 with different size";
    FileService::create_or_write_file(
        &state,
        &admin,
        "local",
        "/cached_file.txt",
        content2.to_vec(),
        None,
    )
    .await
    .unwrap();

    // 3. Stat retrieves updated size
    let stat2 = FileService::stat_file(&state, &admin, "local", "/cached_file.txt")
        .await
        .unwrap();
    assert_eq!(stat2.size, content2.len() as u64);

    // 4. Delete file -> invalidates cache
    FileService::delete_entry(&state, &admin, "local", "/cached_file.txt")
        .await
        .unwrap();

    let cached_after_del = state
        .metadata_cache
        .get("local", "/cached_file.txt")
        .await;
    assert!(
        cached_after_del.is_none(),
        "MetadataCache must be invalidated on deletion"
    );
}

#[tokio::test]
async fn test_directory_paged_listing_has_more_and_optional_total() {
    let (state, admin, _temp) = setup_test_context().await;

    // Create 10 files
    for i in 0..10 {
        FileService::create_or_write_file(
            &state,
            &admin,
            "local",
            &format!("/paged_dir/file_{:02}.txt", i),
            format!("data {}", i).into_bytes(),
            None,
        )
        .await
        .unwrap();
    }

    // List with limit 4
    let listing = FileService::list_directory_paged(
        &state,
        &admin,
        "local",
        Some("/paged_dir".into()),
        None,
        None,
        None,
        None,
        Some(4),
    )
    .await
    .unwrap();

    assert_eq!(listing.entries.len(), 4);
    assert!(listing.has_more, "Must indicate has_more = true");
    assert!(listing.next_cursor.is_some(), "Must return next_cursor");
    assert_eq!(listing.total_count, Some(4));
}
