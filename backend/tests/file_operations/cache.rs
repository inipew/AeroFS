use axum::extract::FromRef;
use backend::{
    application::files::{DeleteEntriesCommand, StatFileCommand, WriteFileCommand},
    domain::ConnectionId,
    state::FileApiState,
};

use crate::support::{transfer_admin_actor, TestAppBuilder};

#[tokio::test]
async fn write_and_delete_invalidate_cached_metadata_before_the_next_stat() {
    let app = TestAppBuilder::new().running().build().await;
    let actor = transfer_admin_actor();
    let files = FileApiState::from_ref(&app.state);
    let connection = ConnectionId::local();
    let path = "/cached-file.txt";

    files
        .files
        .write_file
        .execute(
            &actor,
            WriteFileCommand {
                connection: connection.clone(),
                path: path.into(),
                content: b"version-one".to_vec(),
                expected_etag: None,
                create_only: false,
            },
        )
        .await
        .unwrap();
    let initial = files
        .files
        .stat_file
        .execute(
            &actor,
            StatFileCommand {
                connection: connection.clone(),
                path: path.into(),
            },
        )
        .await
        .unwrap();
    files
        .service
        .cache_metadata("local", path, initial.clone())
        .await;
    assert!(files.service.cached_metadata("local", path).await.is_some());

    let updated_payload = b"version-two-with-a-different-size".to_vec();
    files
        .files
        .write_file
        .execute(
            &actor,
            WriteFileCommand {
                connection: connection.clone(),
                path: path.into(),
                content: updated_payload.clone(),
                expected_etag: None,
                create_only: false,
            },
        )
        .await
        .unwrap();
    assert!(
        files.service.cached_metadata("local", path).await.is_none(),
        "write must evict stale metadata before a subsequent stat"
    );
    let updated = files
        .files
        .stat_file
        .execute(
            &actor,
            StatFileCommand {
                connection: connection.clone(),
                path: path.into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.size, updated_payload.len() as u64);

    files
        .service
        .cache_metadata("local", path, updated)
        .await;
    let deleted = files
        .files
        .delete_entries
        .execute(
            &actor,
            DeleteEntriesCommand {
                connection: connection.clone(),
                paths: vec![path.into()],
            },
        )
        .await
        .unwrap();
    assert!(deleted.failed.is_empty());
    assert_eq!(deleted.succeeded, vec![path.to_string()]);
    assert!(
        files.service.cached_metadata("local", path).await.is_none(),
        "delete must evict cached metadata"
    );
    assert!(files
        .files
        .stat_file
        .execute(
            &actor,
            StatFileCommand {
                connection,
                path: path.into(),
            },
        )
        .await
        .is_err());
}
