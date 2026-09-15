use backend::vfs::cleanup_stale_staging_files;
use std::time::Duration;

#[tokio::test]
async fn orphan_staging_cleanup_removes_only_internal_partial_files() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("storage-root");
    std::fs::create_dir_all(&root).unwrap();
    let legacy_staging = root.join("movie.mkv.aerofs.part");
    let transfer_staging = root.join(".image.png.aerofs-part-job999");
    let regular = root.join("regular-file.txt");
    std::fs::write(&legacy_staging, b"partial-one").unwrap();
    std::fs::write(&transfer_staging, b"partial-two").unwrap();
    std::fs::write(&regular, b"permanent").unwrap();

    let cleaned = cleanup_stale_staging_files(&root, Duration::ZERO)
        .await
        .unwrap();

    assert_eq!(cleaned, 2);
    assert!(!legacy_staging.exists());
    assert!(!transfer_staging.exists());
    assert!(regular.exists());
}

#[tokio::test]
async fn orphan_staging_cleanup_surfaces_storage_scan_errors() {
    let temp = tempfile::tempdir().unwrap();
    let not_a_directory = temp.path().join("storage-root-file");
    std::fs::write(&not_a_directory, b"not a directory").unwrap();

    let error = cleanup_stale_staging_files(&not_a_directory, Duration::ZERO)
        .await
        .expect_err("invalid storage root must surface its scan error");
    assert!(matches!(
        error.kind(),
        std::io::ErrorKind::NotADirectory | std::io::ErrorKind::Other
    ));
}
