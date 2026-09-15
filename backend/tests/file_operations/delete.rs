use axum::extract::FromRef;
use backend::{
    application::files::DeleteEntriesCommand,
    domain::{Actor, ConnectionId},
    state::FileApiState,
};

use crate::support::TestAppBuilder;

#[tokio::test]
async fn batch_delete_tolerates_duplicate_and_nested_paths() {
    let app = TestAppBuilder::new()
        .running()
        .with_file("batch_test/nested/file1.txt", b"1".to_vec())
        .with_file("batch_test/file2.txt", b"2".to_vec())
        .build()
        .await;
    let files = FileApiState::from_ref(&app.state);
    let admin = Actor {
        id: "admin-id".to_string(),
        username: "admin".to_string(),
        is_admin: true,
    };

    let result = files
        .files
        .delete_entries
        .execute(
            &admin,
            DeleteEntriesCommand {
                connection: ConnectionId::local(),
                paths: vec![
                    "/batch_test/nested/file1.txt".to_string(),
                    "/batch_test/nested/file1.txt".to_string(),
                    "/batch_test/nested".to_string(),
                    "/batch_test/file2.txt".to_string(),
                    "/batch_test".to_string(),
                    "/batch_test".to_string(),
                ],
            },
        )
        .await
        .expect("batch delete should execute");

    assert!(
        result.failed.is_empty(),
        "duplicate/nested deletes should be idempotent: {:?}",
        result.failed
    );
    assert!(!result.succeeded.is_empty());
    assert!(!app.storage_path("batch_test").exists());
}
