use axum::extract::FromRef;
use backend::{
    application::files::ListDirectoryCommand,
    domain::ConnectionId,
    services::settings_service::UpdateSettingsRequest,
    state::{FileApiState, SettingsState},
};

use crate::support::{transfer_admin_actor, TestAppBuilder};

#[tokio::test]
async fn committed_local_root_update_is_visible_to_file_listing_immediately() {
    let app = TestAppBuilder::new().running().build().await;
    let actor = transfer_admin_actor();
    let settings = SettingsState::from_ref(&app.state);

    let new_root = app.temp.path().join("switched-root");
    std::fs::create_dir_all(&new_root).unwrap();
    std::fs::write(new_root.join("new-marker.txt"), b"switched root").unwrap();

    settings
        .service
        .update_settings(
            &actor,
            UpdateSettingsRequest {
                settings: None,
                local_root: Some(new_root.to_string_lossy().into_owned()),
                temp_dir: None,
                allow_symlinks: None,
                show_hidden_default: None,
                read_only_default: None,
            },
        )
        .await
        .unwrap();

    let files = FileApiState::from_ref(&app.state);
    let listing = files
        .files
        .list_directory
        .execute(
            &actor,
            ListDirectoryCommand {
                connection: ConnectionId::local(),
                path: None,
                show_hidden: None,
                sort: None,
                order: None,
                cursor: None,
                limit: None,
            },
        )
        .await
        .unwrap();

    assert!(listing.entries.iter().any(|entry| entry.name == "new-marker.txt"));
}
