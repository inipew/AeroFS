use backend::domain::{FileKind, VfsPath};
use backend::vfs::opendal::{build_fs_operator, OpenDalFileSystem};
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
    let src_dir = VfsPath::new("test_opendal", "/documents");
    vfs.create_dir(&src_dir).await.unwrap();

    let root_entries = vfs.list(&root_path).await.unwrap();
    assert_eq!(root_entries.len(), 1);
    assert_eq!(root_entries[0].name, "documents");
    assert_eq!(root_entries[0].kind, FileKind::Directory);

    // 3. Write file stream
    let file_path = VfsPath::new("test_opendal", "/documents/report.txt");
    let content = b"AeroFS OpenDAL Architecture Migration Report".to_vec();
    let cursor = std::io::Cursor::new(content.clone());
    vfs.write_stream(&file_path, Box::new(cursor)).await.unwrap();

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
    let renamed_path = VfsPath::new("test_opendal", "/documents/final_report.txt");
    vfs.rename(&file_path, &renamed_path).await.unwrap();
    assert!(vfs.stat(&file_path).await.is_err());
    assert!(vfs.stat(&renamed_path).await.is_ok());

    // 7. Copy
    let copy_path = VfsPath::new("test_opendal", "/documents/final_report_copy.txt");
    vfs.copy(&renamed_path, &copy_path).await.unwrap();
    assert!(vfs.stat(&copy_path).await.is_ok());

    // 8. Delete
    vfs.delete(&copy_path).await.unwrap();
    assert!(vfs.stat(&copy_path).await.is_err());

    // 9. Root stat
    let root_meta = vfs.stat(&root_path).await.unwrap();
    assert_eq!(root_meta.kind, FileKind::Directory);
}
