use axum::extract::FromRef;
use backend::auth::{AuthenticatedUser, UserInfo};
use backend::bootstrap::build_application;
use backend::config::AppConfig;
use backend::db::init_db;
use backend::domain::{Actor, ConnectionId};
use backend::services::{EditorService, FileService};
use backend::state::{AppState, ArchiveState, RuntimeOwner, ShutdownReason};
use backend::vfs::factory::ProviderFactory;
use backend::vfs::registry::ProviderRegistry;
use std::sync::Arc;
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
    let db_path = temp.path().join("test_plan52.db");
    let storage_dir = temp.path().join("storage");
    std::fs::create_dir_all(&storage_dir).unwrap();

    let mut config = AppConfig::default();
    config.database.url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    config.filesystem.default_local_root = storage_dir;

    let pool = init_db(&config.database.url).await.unwrap();
    let built = build_application(config, pool).await;

    let admin = AuthenticatedUser(UserInfo {
        id: "admin-plan52".into(),
        username: "admin".into(),
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

fn actor_from_user(user: &AuthenticatedUser) -> Actor {
    Actor {
        id: user.id().to_string(),
        username: user.username().to_string(),
        is_admin: user.is_admin(),
    }
}

#[tokio::test]
async fn test_archive_targz_streaming_zero_ram_buffering() {
    let (state, admin, _temp) = setup_test_context().await;
    let archive = ArchiveState::from_ref(&state);
    let actor = actor_from_user(&admin);
    let connection = ConnectionId::local();

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

    let compress_res = archive
        .service
        .compress(
            &actor,
            &connection,
            "/src_archive",
            &["file1.txt".into(), "file2.txt".into()],
            "/packed_archive.tar.gz",
            Some("targz"),
        )
        .await
        .unwrap();
    assert!(compress_res.success);

    let extract_res = archive
        .service
        .extract(
            &actor,
            &connection,
            "/packed_archive.tar.gz",
            "/extracted_dest",
            Some("targz"),
            backend::filesystem::archive::ArchiveOverwriteMode::Overwrite,
        )
        .await
        .unwrap();
    assert!(extract_res.success);

    let (read_f1, _) =
        EditorService::read_for_editing(&state, &admin, "local", "/extracted_dest/file1.txt")
            .await
            .unwrap();
    assert_eq!(read_f1.as_bytes(), f1_data);

    let (read_f2, _) =
        EditorService::read_for_editing(&state, &admin, "local", "/extracted_dest/file2.txt")
            .await
            .unwrap();
    assert_eq!(read_f2.as_bytes(), f2_data);
}

#[tokio::test]
async fn test_storage_runtime_shared_concurrency() {
    let temp = tempdir().unwrap();
    let provider = ProviderFactory::build_local("local", temp.path().to_path_buf()).unwrap();
    let registry = ProviderRegistry::new();
    registry.register("local".to_string(), provider).await;

    let runtime = registry.get_runtime("local").await.unwrap();
    assert_eq!(runtime.connection_id, "local");
    assert!(runtime.capabilities().read);

    let mut handles = Vec::new();
    for _ in 0..10 {
        let rt = Arc::clone(&runtime);
        handles.push(tokio::spawn(async move {
            let _permit = rt.acquire_permit().await.unwrap();
            tokio::time::sleep(Duration::from_millis(10)).await;
        }));
    }

    for handle in handles {
        handle.await.unwrap();
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

    let stat1 = FileService::stat_file(&state, &admin, "local", "/cached_file.txt")
        .await
        .unwrap();
    assert_eq!(stat1.size, content1.len() as u64);

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

    let stat2 = FileService::stat_file(&state, &admin, "local", "/cached_file.txt")
        .await
        .unwrap();
    assert_eq!(
        stat2.size,
        content2.len() as u64,
        "stat after write must not return stale cached metadata"
    );

    FileService::delete_entry(&state, &admin, "local", "/cached_file.txt")
        .await
        .unwrap();
    assert!(
        FileService::stat_file(&state, &admin, "local", "/cached_file.txt")
            .await
            .is_err(),
        "stat after delete must not return stale cached metadata"
    );
}

#[tokio::test]
async fn test_directory_paged_listing_has_more_and_total_count() {
    let (state, admin, _temp) = setup_test_context().await;

    for i in 0..10 {
        FileService::create_or_write_file(
            &state,
            &admin,
            "local",
            &format!("/paged_dir/file_{i:02}.txt"),
            format!("data {i}").into_bytes(),
            None,
        )
        .await
        .unwrap();
    }

    let listing = FileService::list_directory_paged(
        &state,
        &admin,
        "local",
        Some("/paged_dir".into()),
        None,
        Some("name"),
        Some("asc"),
        None,
        Some(4),
    )
    .await
    .unwrap();

    assert_eq!(listing.entries.len(), 4);
    assert!(listing.has_more, "Must indicate has_more = true");
    assert!(listing.next_cursor.is_some(), "Must return next_cursor");
    assert_eq!(
        listing.total_count,
        Some(10),
        "total_count must describe the full filtered directory, not the page length"
    );
}
