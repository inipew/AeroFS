use axum::extract::FromRef;
use backend::{
    auth::{AuthenticatedUser, UserInfo},
    services::trash_service::MoveToTrashRequest,
    state::TrashState,
};

use crate::support::TestAppBuilder;

async fn admin(app: &crate::support::TestApp) -> AuthenticatedUser {
    let (id, username): (String, String) =
        sqlx::query_as("SELECT id, username FROM users WHERE username = 'admin'")
            .fetch_one(&app.db)
            .await
            .unwrap();
    AuthenticatedUser(UserInfo {
        id,
        username,
        is_admin: true,
    })
}

#[tokio::test]
async fn trash_move_list_and_restore_round_trip_preserves_file() {
    let app = TestAppBuilder::new()
        .running()
        .with_file("trash-me.txt", b"trash payload".to_vec())
        .build()
        .await;
    let admin = admin(&app).await;
    let trash = TrashState::from_ref(&app.state);

    let moved = trash
        .service
        .move_to_trash(
            &admin,
            MoveToTrashRequest {
                connection_id: "local".into(),
                paths: vec!["/trash-me.txt".into()],
            },
        )
        .await
        .unwrap();
    assert_eq!(moved.len(), 1);
    assert!(!app.storage_path("trash-me.txt").exists());

    let items = trash.service.list_trash(&admin).await.unwrap();
    assert_eq!(items.len(), 1);
    trash.service.restore_item(&admin, &items[0].id).await.unwrap();
    assert_eq!(std::fs::read(app.storage_path("trash-me.txt")).unwrap(), b"trash payload");
}
