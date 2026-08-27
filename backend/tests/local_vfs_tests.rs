use backend::domain::{FileKind, VfsPath};
use backend::vfs::opendal::{build_fs_operator, OpenDalFileSystem};
use backend::vfs::FileSystem;
use tempfile::tempdir;
use tokio::io::AsyncReadExt;

#[tokio::test]
async fn test_local_vfs_full_workflow() {
    let temp = tempdir().unwrap();
    let root_str = temp.path().to_string_lossy().to_string();
    let op = build_fs_operator(&root_str).unwrap();
    let vfs = OpenDalFileSystem::new("test_conn", op);

    // 1. Initial listing of empty root
    let root_path = VfsPath::root("test_conn");
    let entries = vfs.list(&root_path).await.unwrap();
    assert_eq!(entries.len(), 0);

    // 2. Create nested directories
    let src_dir = VfsPath::new("test_conn", "/src/components");
    vfs.create_dir(&src_dir).await.unwrap();

    // 3. Write file stream
    let file_path = VfsPath::new("test_conn", "/src/components/Button.vue");
    let content = b"<template><button>Click me</button></template>".to_vec();
    let cursor = std::io::Cursor::new(content.clone());
    vfs.write_stream(&file_path, Box::new(cursor)).await.unwrap();

    // 4. Verify file metadata & stat
    let meta = vfs.stat(&file_path).await.unwrap();
    assert_eq!(meta.name, "Button.vue");
    assert_eq!(meta.size, content.len() as u64);
    assert_eq!(meta.kind, FileKind::File);
    assert_eq!(meta.is_readonly, false);
    assert!(!meta.etag.is_empty());

    // 5. Read back stream
    let mut reader = vfs.read_stream(&file_path).await.unwrap();
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer).await.unwrap();
    assert_eq!(buffer, content);

    // 6. Overwrite file
    let new_content = b"<template><button class='primary'>Updated</button></template>".to_vec();
    let cursor2 = std::io::Cursor::new(new_content.clone());
    vfs.write_stream(&file_path, Box::new(cursor2)).await.unwrap();

    let meta2 = vfs.stat(&file_path).await.unwrap();
    assert_eq!(meta2.size, new_content.len() as u64);

    // 7. Copy file
    let copied_btn = VfsPath::new("test_conn", "/src/components/ButtonCopy.vue");
    vfs.copy(&file_path, &copied_btn).await.unwrap();
    assert!(vfs.stat(&copied_btn).await.is_ok());

    // 8. Rename / Move
    let moved_btn = VfsPath::new("test_conn", "/src/components/CustomButton.vue");
    vfs.rename(&copied_btn, &moved_btn).await.unwrap();
    assert!(vfs.stat(&copied_btn).await.is_err());
    assert!(vfs.stat(&moved_btn).await.is_ok());

    // 9. Delete
    vfs.delete(&moved_btn).await.unwrap();
    assert!(vfs.stat(&moved_btn).await.is_err());
}
