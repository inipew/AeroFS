use axum::extract::FromRef;
use backend::{
    domain::{ChecksumCapabilities, FileKind, SortField, SortOrder, VfsPath},
    errors::VfsError,
    state::FileApiState,
    transfer::{
        TransferJob, TransferPhase, TransferPlanner, TransferStatus, TransferStrategy, TransferType,
    },
    vfs::{
        opendal::{
            build_fs_operator, build_fs_operator_with_config, build_s3_operator,
            OpenDalFileSystem,
        },
        FileSystem,
    },
};
use chrono::Utc;
use std::sync::Arc;
use tokio::io::AsyncReadExt;

use crate::support::{transfer_admin_actor, TestAppBuilder};

fn transfer_job(
    id: &str,
    transfer_type: TransferType,
    source_connection: &str,
    source_path: &str,
    destination_connection: &str,
    destination_path: &str,
) -> TransferJob {
    let now = Utc::now();
    TransferJob {
        id: id.to_string(),
        user_id: Some("test-user".to_string()),
        name: "provider-contract".to_string(),
        transfer_type,
        source_connection_id: source_connection.to_string(),
        source_path: source_path.to_string(),
        destination_connection_id: destination_connection.to_string(),
        destination_path: destination_path.to_string(),
        status: TransferStatus::Queued,
        phase: TransferPhase::Preparing,
        execution_mode: Default::default(),
        staging: Default::default(),
        transferred_bytes: 0,
        total_bytes: 100,
        speed_bytes_per_sec: 0,
        eta_seconds: None,
        checksum: None,
        error_message: None,
        dismissed_at: None,
        created_at: now,
        updated_at: now,
    }
}

#[tokio::test]
async fn local_provider_crud_range_and_root_metadata_are_consistent() {
    let temp = tempfile::tempdir().unwrap();
    let op = build_fs_operator(&temp.path().to_string_lossy()).unwrap();
    let vfs = OpenDalFileSystem::new("local-test", op);
    let root = VfsPath::root("local-test");

    assert!(vfs.list(&root).await.unwrap().is_empty());

    let dir = VfsPath::new("local-test", "/documents").unwrap();
    vfs.create_dir(&dir).await.unwrap();
    let file = VfsPath::new("local-test", "/documents/report.txt").unwrap();
    let content = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ".to_vec();
    vfs.write_stream(&file, Box::new(std::io::Cursor::new(content.clone())))
        .await
        .unwrap();

    let metadata = vfs.stat(&file).await.unwrap();
    assert_eq!(metadata.kind, FileKind::File);
    assert_eq!(metadata.size, content.len() as u64);
    assert!(!metadata.etag.is_empty());

    let mut reader = vfs.read_stream(&file).await.unwrap();
    let mut full = Vec::new();
    reader.read_to_end(&mut full).await.unwrap();
    assert_eq!(full, content);

    let mut range = vfs.read_range(&file, 10, 5).await.unwrap();
    let mut partial = Vec::new();
    range.read_to_end(&mut partial).await.unwrap();
    assert_eq!(partial, b"ABCDE");

    let copied = VfsPath::new("local-test", "/documents/copy.txt").unwrap();
    vfs.copy(&file, &copied).await.unwrap();
    let renamed = VfsPath::new("local-test", "/documents/final.txt").unwrap();
    vfs.rename(&copied, &renamed).await.unwrap();
    assert!(vfs.stat(&copied).await.is_err());
    assert!(vfs.stat(&renamed).await.is_ok());
    vfs.delete(&renamed).await.unwrap();
    assert!(vfs.stat(&renamed).await.is_err());

    let root_metadata = vfs.stat(&root).await.unwrap();
    assert_eq!(root_metadata.kind, FileKind::Directory);
    assert_eq!(root_metadata.path, "/");
    assert_eq!(root_metadata.created_at, None);
    assert!(!root_metadata.etag.is_empty());
}

#[test]
fn vfs_path_rejects_parent_traversal_before_provider_io() {
    for path in ["../../etc/passwd", "/documents/../../shadow", "/../secret.key"] {
        assert!(matches!(
            VfsPath::new("local-test", path),
            Err(VfsError::InvalidPath(_))
        ));
    }
}

#[tokio::test]
async fn unix_permissions_round_trip_through_local_provider() {
    #[cfg(unix)]
    {
        let temp = tempfile::tempdir().unwrap();
        let op = build_fs_operator(&temp.path().to_string_lossy()).unwrap();
        let vfs = OpenDalFileSystem::new_local("local", op, temp.path());
        let file = VfsPath::new("local", "/secret.key").unwrap();
        vfs.write_stream(&file, Box::new(std::io::Cursor::new(b"secret".to_vec())))
            .await
            .unwrap();

        vfs.set_permissions(&file, "0600").await.unwrap();
        assert_eq!(vfs.stat(&file).await.unwrap().permissions.as_deref(), Some("0600"));
        let entry = vfs
            .list(&VfsPath::root("local"))
            .await
            .unwrap()
            .into_iter()
            .find(|entry| entry.name == "secret.key")
            .unwrap();
        assert_eq!(entry.permissions.as_deref(), Some("0600"));

        vfs.set_permissions(&file, "0750").await.unwrap();
        assert_eq!(vfs.stat(&file).await.unwrap().permissions.as_deref(), Some("0750"));
    }
}

#[tokio::test]
async fn provider_capabilities_and_presign_support_match_backend_semantics() {
    let temp = tempfile::tempdir().unwrap();
    let local_op = build_fs_operator(&temp.path().to_string_lossy()).unwrap();
    let local = OpenDalFileSystem::new("local", local_op);
    let local_caps = local.capabilities();
    assert!(!local_caps.resume_upload);
    assert!(local_caps.native_copy);
    assert!(!local_caps.server_side_copy);
    assert!(local.as_presign().is_none());

    let s3_op = build_s3_operator(
        "test-bucket",
        Some("us-east-1"),
        None,
        Some("ak"),
        Some("sk"),
        None,
    )
    .unwrap();
    let s3 = OpenDalFileSystem::new("s3", s3_op);
    let s3_caps = s3.capabilities();
    assert!(s3_caps.checksum);
    assert!(s3_caps.server_side_copy);
    assert!(s3_caps.range_read);
    assert!(s3.as_presign().is_some());

    let url = s3
        .as_presign()
        .unwrap()
        .presign_read_url(
            &VfsPath::new("s3", "/data.bin").unwrap(),
            std::time::Duration::from_secs(300),
        )
        .await
        .unwrap();
    assert!(url.contains("test-bucket") || url.contains("data.bin") || url.contains("X-Amz"));
}

#[tokio::test]
async fn directory_listing_cursor_pages_are_stable_and_complete() {
    let mut builder = TestAppBuilder::new().running();
    for index in 0..25usize {
        builder = builder.with_file(
            format!("page_item_{index:02}.txt"),
            format!("content-{index}").into_bytes(),
        );
    }
    let app = builder.build().await;
    let actor = transfer_admin_actor();
    let files = FileApiState::from_ref(&app.state);

    let page = |cursor: Option<String>| {
        let files = files.clone();
        let actor = actor.clone();
        async move {
            files
                .files
                .list_directory
                .execute(
                    &actor,
                    backend::application::files::ListDirectoryCommand {
                        connection: backend::domain::ConnectionId::local(),
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
    };

    let first = page(None).await;
    let second = page(first.next_cursor.clone()).await;
    let third = page(second.next_cursor.clone()).await;
    assert_eq!((first.entries.len(), second.entries.len(), third.entries.len()), (10, 10, 5));
    assert!(first.next_cursor.is_some());
    assert!(second.next_cursor.is_some());
    assert!(third.next_cursor.is_none());

    let names: std::collections::HashSet<_> = first
        .entries
        .into_iter()
        .chain(second.entries)
        .chain(third.entries)
        .map(|entry| entry.name)
        .collect();
    assert_eq!(names.len(), 25);
}

#[tokio::test]
async fn transfer_planner_selects_native_streaming_and_server_side_paths_from_provider_capabilities() {
    let temp = tempfile::tempdir().unwrap();
    let local_op = build_fs_operator(&temp.path().to_string_lossy()).unwrap();
    let local: Arc<dyn FileSystem> = Arc::new(OpenDalFileSystem::new("local", local_op));
    let s3_op = build_s3_operator("bucket", None, None, None, None, None).unwrap();
    let s3: Arc<dyn FileSystem> = Arc::new(OpenDalFileSystem::new("s3", s3_op));

    let local_src = VfsPath::new("local", "/a").unwrap();
    let local_dst = VfsPath::new("local", "/b").unwrap();
    assert_eq!(
        TransferPlanner::plan_transfer(
            &transfer_job("move", TransferType::Move, "local", "/a", "local", "/b"),
            &local,
            &local,
            &local_src,
            &local_dst,
        ),
        TransferStrategy::NativeRename
    );

    let s3_dst = VfsPath::new("s3", "/b").unwrap();
    assert_eq!(
        TransferPlanner::plan_transfer(
            &transfer_job("cross", TransferType::Copy, "local", "/a", "s3", "/b"),
            &local,
            &s3,
            &local_src,
            &s3_dst,
        ),
        TransferStrategy::Streaming
    );

    let s3_src = VfsPath::new("s3", "/a").unwrap();
    assert_eq!(
        TransferPlanner::plan_transfer(
            &transfer_job("s3-copy", TransferType::Copy, "s3", "/a", "s3", "/b"),
            &s3,
            &s3,
            &s3_src,
            &s3_dst,
        ),
        TransferStrategy::ServerSideCopy
    );
}

#[tokio::test]
async fn configured_local_operator_and_checksum_capabilities_remain_usable() {
    let config = backend::config::ProviderStorageConfig {
        max_concurrency: 32,
        control_timeout_secs: 12,
        io_timeout_secs: 90,
        retry_attempts: 2,
    };
    let temp = tempfile::tempdir().unwrap();
    let op = build_fs_operator_with_config(&temp.path().to_string_lossy(), Some(&config)).unwrap();
    let vfs = OpenDalFileSystem::new("configured", op);
    let file = VfsPath::new("configured", "/hello.txt").unwrap();
    vfs.create_file(&file).await.unwrap();
    assert!(vfs.stat(&file).await.is_ok());

    let s3 = ChecksumCapabilities::s3_default();
    assert!(s3.md5 && s3.crc32 && s3.crc32c && s3.sha1 && s3.sha256);
    assert!(ChecksumCapabilities::all().has_any());
    assert!(!ChecksumCapabilities::default().has_any());
}
