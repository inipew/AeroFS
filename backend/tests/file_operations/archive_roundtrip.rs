use axum::extract::FromRef;
use backend::{
    domain::ConnectionId,
    filesystem::archive::ArchiveOverwriteMode,
    state::ArchiveState,
};

use crate::support::{transfer_admin_actor, TestAppBuilder};

#[tokio::test]
async fn targz_compress_and_extract_round_trip_preserves_payloads() {
    let first = b"first targz payload".to_vec();
    let second = b"second targz payload".to_vec();
    let app = TestAppBuilder::new()
        .running()
        .with_file("src_archive/file1.txt", first.clone())
        .with_file("src_archive/file2.txt", second.clone())
        .build()
        .await;
    let actor = transfer_admin_actor();
    let archive = ArchiveState::from_ref(&app.state);
    let connection = ConnectionId::local();

    let compressed = archive
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
    assert!(compressed.success);

    let extracted = archive
        .service
        .extract(
            &actor,
            &connection,
            "/packed_archive.tar.gz",
            "/extracted_dest",
            Some("targz"),
            ArchiveOverwriteMode::Overwrite,
        )
        .await
        .unwrap();
    assert!(extracted.success);
    assert_eq!(
        std::fs::read(app.storage_path("extracted_dest/file1.txt")).unwrap(),
        first
    );
    assert_eq!(
        std::fs::read(app.storage_path("extracted_dest/file2.txt")).unwrap(),
        second
    );
}
