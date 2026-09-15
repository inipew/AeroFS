use axum::extract::FromRef;
use backend::{
    application::files::CompletePresignedCommand,
    domain::ConnectionId,
    errors::AppError,
    state::FileApiState,
};

use crate::support::{transfer_admin_actor, TestAppBuilder};

#[tokio::test]
async fn complete_presigned_validates_existing_upload_size_and_missing_target() {
    let app = TestAppBuilder::new()
        .running()
        .with_file("presigned-upload.bin", vec![1, 2, 3, 4, 5])
        .build()
        .await;
    let actor = transfer_admin_actor();
    let files = FileApiState::from_ref(&app.state);

    let metadata = files
        .files
        .complete_presigned
        .execute(
            &actor,
            CompletePresignedCommand {
                connection: ConnectionId::local(),
                path: "/presigned-upload.bin".into(),
                expected_size: Some(5),
                expected_checksum: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(metadata.size, 5);
    assert_eq!(metadata.name, "presigned-upload.bin");

    let size_mismatch = files
        .files
        .complete_presigned
        .execute(
            &actor,
            CompletePresignedCommand {
                connection: ConnectionId::local(),
                path: "/presigned-upload.bin".into(),
                expected_size: Some(999),
                expected_checksum: None,
            },
        )
        .await
        .expect_err("size mismatch must fail presigned-upload completion");
    assert!(matches!(size_mismatch, AppError::BadRequest(message) if message.contains("size mismatch")));

    let missing = files
        .files
        .complete_presigned
        .execute(
            &actor,
            CompletePresignedCommand {
                connection: ConnectionId::local(),
                path: "/missing-presigned-upload.bin".into(),
                expected_size: None,
                expected_checksum: None,
            },
        )
        .await
        .expect_err("missing uploaded object must not be accepted as completed");
    assert!(matches!(missing, AppError::NotFound(_)));
}
