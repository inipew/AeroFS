use axum::extract::FromRef;
use backend::auth::{AuthenticatedUser, UserInfo};
use backend::bootstrap::build_application;
use backend::config::AppConfig;
use backend::db::init_db;
use backend::domain::{Actor, ConnectionId};
use backend::services::EditorService;
use backend::state::{AppState, ArchiveState, FileApiState, RuntimeOwner, ShutdownReason};
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

async fn write_file(
    state: &AppState,
    user: &AuthenticatedUser,
    path: &str,
    content: Vec<u8>,
) -> backend::domain::FileMetadata {
    let file_api = FileApiState::from_ref(state);
    file_api
        .files
        .write_file
        .execute(
            &actor_from_user(user),
            backend::application::files::WriteFileCommand {
                connection: ConnectionId::local(),
                path: path.to_string(),
                content,
                expected_etag: None,
                create_only: false,
            },
        )
        .await
        .unwrap()
}

async fn stat_file(
    state: &AppState,
    user: &AuthenticatedUser,
    path: &str,
) -> Result<backend::domain::FileMetadata, backend::errors::AppError> {
    let file_api = FileApiState::from_ref(state);
    if let Some(metadata) = file_api.service.cached_metadata("local", path).await {
        return Ok(metadata);
    }
    let metadata = file_api
        .files
        .stat_file
        .execute(
            &actor_from_user(user),
            backend::application::files::StatFileCommand {
                connection: ConnectionId::local(),
                path: path.to_string(),
            },
        )
        .await?;
    file_api
        .service
        .cache_metadata("local", path, metadata.clone())
        .await;
    Ok(metadata)
}

#[tokio::test]
async fn test_archive_targz_streaming_zero_ram_buffering() {
    let (state, admin, _temp) = setup_test_context().await;
    let archive = ArchiveState::from_ref(&state);
    let file_api = FileApiState::from_ref(&state);
    let actor = actor_from_user(&admin);
    let connection = ConnectionId::local();

    let f1_data = b"Hello Plan 52 TAR.GZ Streaming Compression!";
    let f2_data = b"Second file to be archived inside the streaming archive";
    write_file(
        &state,
        &admin,
        "/src_archive/file1.txt",
        f1_data.to_vec(),
    )
    .await;

    write_file(
        &state,
        &admin,
        "/src_archive/file2.txt",
        f2_data.to_vec(),
    )
    .await;

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
        EditorService::read_for_editing(&file_api, &admin, "local", "/extracted_dest/file1.txt")
            .await
            .unwrap();
    assert_eq!(read_f1.as_bytes(), f1_data);

    let (read_f2, _) =
        EditorService::read_for_editing(&file_api, &admin, "local", "/extracted_dest/file2.txt")
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
    let actor = actor_from_user(&admin);
    let connection = ConnectionId::local();
    let file_api = FileApiState::from_ref(&state);

    let content = b"PRESIGNED PAYLOAD FOR VALIDATION TEST";
    write_file(
        &state,
        &admin,
        "/presigned_valid.dat",
        content.to_vec(),
    )
    .await;

    let meta = file_api
        .files
        .complete_presigned
        .execute(
            &actor,
            backend::application::files::CompletePresignedCommand {
                connection: connection.clone(),
                path: "/presigned_valid.dat".to_string(),
                expected_size: Some(content.len() as u64),
                expected_checksum: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(meta.size, content.len() as u64);

    let err_size = file_api
        .files
        .complete_presigned
        .execute(
            &actor,
            backend::application::files::CompletePresignedCommand {
                connection: connection.clone(),
                path: "/presigned_valid.dat".to_string(),
                expected_size: Some(99999),
                expected_checksum: None,
            },
        )
        .await;
    assert!(err_size.is_err(), "Size mismatch must fail verification");

    let err_nf = file_api
        .files
        .complete_presigned
        .execute(
            &actor,
            backend::application::files::CompletePresignedCommand {
                connection,
                path: "/non_existent.dat".to_string(),
                expected_size: None,
                expected_checksum: None,
            },
        )
        .await;
    assert!(err_nf.is_err(), "Non-existent path must fail");
}

#[tokio::test]
async fn test_metadata_cache_lifecycle_and_invalidation() {
    let (state, admin, _temp) = setup_test_context().await;
    let file_api = FileApiState::from_ref(&state);

    let content1 = b"Original Content v1";
    write_file(
        &state,
        &admin,
        "/cached_file.txt",
        content1.to_vec(),
    )
    .await;

    let stat1 = stat_file(&state, &admin, "/cached_file.txt").await.unwrap();
    assert_eq!(stat1.size, content1.len() as u64);

    let content2 = b"Updated Content v2 with different size";
    write_file(
        &state,
        &admin,
        "/cached_file.txt",
        content2.to_vec(),
    )
    .await;

    let stat2 = stat_file(&state, &admin, "/cached_file.txt").await.unwrap();
    assert_eq!(
        stat2.size,
        content2.len() as u64,
        "stat after write must not return stale cached metadata"
    );

    let delete_result = file_api
        .files
        .delete_entries
        .execute(
            &actor_from_user(&admin),
            backend::application::files::DeleteEntriesCommand {
                connection: ConnectionId::local(),
                paths: vec!["/cached_file.txt".to_string()],
            },
        )
        .await
        .unwrap();
    assert!(delete_result.failed.is_empty());
    assert!(!delete_result.succeeded.is_empty());
    assert!(
        stat_file(&state, &admin, "/cached_file.txt").await.is_err(),
        "stat after delete must not return stale cached metadata"
    );
}

#[tokio::test]
async fn test_directory_paged_listing_has_more_and_total_count() {
    let (state, admin, _temp) = setup_test_context().await;
    let file_api = FileApiState::from_ref(&state);

    for i in 0..10 {
        write_file(
            &state,
            &admin,
            &format!("/paged_dir/file_{i:02}.txt"),
            format!("data {i}").into_bytes(),
        )
        .await;
    }

    let listing = file_api
        .files
        .list_directory
        .execute(
            &actor_from_user(&admin),
            backend::application::files::ListDirectoryCommand {
                connection: ConnectionId::local(),
                path: Some("/paged_dir".into()),
                show_hidden: None,
                sort: Some(backend::domain::SortField::Name),
                order: Some(backend::domain::SortOrder::Asc),
                cursor: None,
                limit: Some(4),
            },
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
