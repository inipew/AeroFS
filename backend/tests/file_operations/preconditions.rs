use axum::extract::FromRef;
use backend::{
    application::files::WriteFileCommand,
    domain::{Actor, ConnectionId},
    errors::AppError,
    state::FileApiState,
};

use crate::support::TestAppBuilder;

#[tokio::test]
async fn if_match_on_missing_target_is_rejected_as_precondition_failure() {
    let app = TestAppBuilder::new().running().build().await;
    let files = FileApiState::from_ref(&app.state);
    let admin = Actor {
        id: "admin-id".to_string(),
        username: "admin".to_string(),
        is_admin: true,
    };

    let result = files
        .files
        .write_file
        .execute(
            &admin,
            WriteFileCommand {
                connection: ConnectionId::local(),
                path: "/missing.txt".to_string(),
                content: b"must not create".to_vec(),
                expected_etag: Some("\"some-etag\"".to_string()),
                create_only: false,
            },
        )
        .await;

    assert!(matches!(result, Err(AppError::PreconditionFailed(_))));
    assert!(!app.storage_path("missing.txt").exists());
}
