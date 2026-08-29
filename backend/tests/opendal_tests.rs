use backend::domain::{FileKind, VfsPath};
use backend::errors::VfsError;
use backend::vfs::opendal::{build_fs_operator, build_s3_operator, OpenDalFileSystem};
use backend::vfs::FileSystem;
use tempfile::tempdir;
use tokio::io::AsyncReadExt;

#[tokio::test]
async fn test_opendal_fs_full_crud_workflow() {
    let temp = tempdir().unwrap();
    let root_str = temp.path().to_string_lossy().to_string();
    let op = build_fs_operator(&root_str).unwrap();
    let vfs = OpenDalFileSystem::new("test_opendal", op);

    // 1. Initial listing of empty root
    let root_path = VfsPath::root("test_opendal");
    let entries = vfs.list(&root_path).await.unwrap();
    assert_eq!(entries.len(), 0);

    // 2. Create nested directory
    let src_dir = VfsPath::new("test_opendal", "/documents").unwrap();
    vfs.create_dir(&src_dir).await.unwrap();

    let root_entries = vfs.list(&root_path).await.unwrap();
    assert_eq!(root_entries.len(), 1);
    assert_eq!(root_entries[0].name, "documents");
    assert_eq!(root_entries[0].kind, FileKind::Directory);

    // 3. Write file stream
    let file_path = VfsPath::new("test_opendal", "/documents/report.txt").unwrap();
    let content = b"AeroFS OpenDAL Architecture Migration Report".to_vec();
    let cursor = std::io::Cursor::new(content.clone());
    vfs.write_stream(&file_path, Box::new(cursor))
        .await
        .unwrap();

    // 4. Stat file and verify metadata
    let meta = vfs.stat(&file_path).await.unwrap();
    assert_eq!(meta.name, "report.txt");
    assert_eq!(meta.size, content.len() as u64);
    assert_eq!(meta.kind, FileKind::File);
    assert!(!meta.etag.is_empty());

    // 5. Read back stream and verify content
    let mut reader = vfs.read_stream(&file_path).await.unwrap();
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer).await.unwrap();
    assert_eq!(buffer, content);

    // 6. Rename / Move
    let renamed_path = VfsPath::new("test_opendal", "/documents/final_report.txt").unwrap();
    vfs.rename(&file_path, &renamed_path).await.unwrap();
    assert!(vfs.stat(&file_path).await.is_err());
    assert!(vfs.stat(&renamed_path).await.is_ok());

    // 7. Copy
    let copy_path = VfsPath::new("test_opendal", "/documents/final_report_copy.txt").unwrap();
    vfs.copy(&renamed_path, &copy_path).await.unwrap();
    assert!(vfs.stat(&copy_path).await.is_ok());

    // 8. Delete
    vfs.delete(&copy_path).await.unwrap();
    assert!(vfs.stat(&copy_path).await.is_err());

    // 9. Root stat
    let root_meta = vfs.stat(&root_path).await.unwrap();
    assert_eq!(root_meta.kind, FileKind::Directory);
}

#[tokio::test]
async fn test_opendal_strict_path_traversal_rejection() {
    let temp = tempdir().unwrap();
    let root_str = temp.path().to_string_lossy().to_string();
    let op = build_fs_operator(&root_str).unwrap();
    let vfs = OpenDalFileSystem::new("test_conn", op);

    // Verify root is accessible
    let root_path = VfsPath::root("test_conn");
    assert!(vfs.stat(&root_path).await.is_ok());

    // P0 #1: Traversal paths must be strictly rejected
    let traversal_paths = vec![
        "../../etc/passwd",
        "/documents/../../shadow",
        "/../secret.key",
    ];

    for tp in traversal_paths {
        let vfs_path_res = VfsPath::new("test_conn", tp);
        assert!(
            matches!(vfs_path_res, Err(VfsError::InvalidPath(_))),
            "Expected InvalidPath for traversal '{}', got {:?}",
            tp,
            vfs_path_res
        );
    }
}

#[tokio::test]
async fn test_opendal_honest_root_stat() {
    let temp = tempdir().unwrap();
    let root_str = temp.path().to_string_lossy().to_string();
    let op = build_fs_operator(&root_str).unwrap();
    let vfs = OpenDalFileSystem::new("test_conn", op);

    let root_path = VfsPath::root("test_conn");
    let meta = vfs.stat(&root_path).await.unwrap();

    // P0 #2: Honest root metadata
    assert_eq!(meta.kind, FileKind::Directory);
    assert_eq!(meta.path, "/");
    assert_eq!(meta.created_at, None);
    assert_eq!(meta.permissions, None);
    assert!(!meta.etag.is_empty());
}

#[tokio::test]
async fn test_opendal_honest_capabilities() {
    let temp = tempdir().unwrap();
    let root_str = temp.path().to_string_lossy().to_string();
    let op = build_fs_operator(&root_str).unwrap();
    let vfs = OpenDalFileSystem::new("test_conn", op);

    let caps = vfs.capabilities();
    // Honest capabilities
    assert!(!caps.resume_upload);
    assert!(caps.native_copy);
    assert!(!caps.server_side_copy);

    // Local provider policy
    #[cfg(unix)]
    assert!(caps.permissions);
}

#[tokio::test]
async fn test_opendal_s3_capabilities() {
    let op = build_s3_operator("test-bucket", Some("us-east-1"), None, None, None, None).unwrap();
    let vfs = OpenDalFileSystem::new("test_s3", op);

    let caps = vfs.capabilities();
    // P2 #1: S3 specific capabilities
    assert!(caps.checksum);
    assert!(caps.server_side_copy);
    assert!(caps.range_read);
}

#[tokio::test]
async fn test_opendal_read_range() {
    let temp = tempdir().unwrap();
    let root_str = temp.path().to_string_lossy().to_string();
    let op = build_fs_operator(&root_str).unwrap();
    let vfs = OpenDalFileSystem::new("test_conn", op);

    let file_path = VfsPath::new("test_conn", "/sample.bin").unwrap();
    let content = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let cursor = std::io::Cursor::new(content.to_vec());
    vfs.write_stream(&file_path, Box::new(cursor))
        .await
        .unwrap();

    // 1. Read slice from offset 10 with length 5 (expecting "ABCDE")
    let mut reader = vfs.read_range(&file_path, 10, 5).await.unwrap();
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf).await.unwrap();
    assert_eq!(buf, b"ABCDE");

    // 2. Read tail from offset 30 with length 10 (expecting "UVWXYZ", 6 bytes)
    let mut reader2 = vfs.read_range(&file_path, 30, 10).await.unwrap();
    let mut buf2 = Vec::new();
    reader2.read_to_end(&mut buf2).await.unwrap();
    assert_eq!(buf2, b"UVWXYZ");
}
