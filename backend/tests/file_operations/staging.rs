use axum::extract::FromRef;
use backend::{
    application::files::ListDirectoryCommand,
    domain::ConnectionId,
    state::FileApiState,
};

use crate::support::{transfer_admin_actor, TestAppBuilder};

#[tokio::test]
async fn internal_transfer_staging_files_never_leak_into_directory_listing() {
    let app = TestAppBuilder::new()
        .running()
        .with_file("visible_file.txt", b"visible".to_vec())
        .with_file(
            ".visible_file.txt.aerofs-part-job1234",
            b"internal staging payload".to_vec(),
        )
        .with_file(
            "staging.dat.aerofs-part-job999",
            b"internal staging payload 2".to_vec(),
        )
        .build()
        .await;
    let files = FileApiState::from_ref(&app.state);
    let actor = transfer_admin_actor();

    let listing = files
        .files
        .list_directory
        .execute(
            &actor,
            ListDirectoryCommand {
                connection: ConnectionId::local(),
                path: Some("/".to_string()),
                show_hidden: Some(true),
                sort: None,
                order: None,
                cursor: None,
                limit: None,
            },
        )
        .await
        .unwrap();

    let names: Vec<_> = listing.entries.into_iter().map(|entry| entry.name).collect();
    assert!(names.iter().any(|name| name == "visible_file.txt"));
    assert!(
        names.iter().all(|name| !name.contains(".aerofs-part-")),
        "internal transfer staging files must remain hidden even when show_hidden=true: {names:?}"
    );
}
