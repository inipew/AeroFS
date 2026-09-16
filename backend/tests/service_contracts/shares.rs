use axum::extract::FromRef;
use backend::{
    auth::{AuthenticatedUser, UserInfo},
    services::share_service::CreateShareRequest,
    state::ShareState,
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
async fn share_create_public_resolve_and_delete_round_trip() {
    let app = TestAppBuilder::new()
        .running()
        .with_file("shared.txt", b"share payload".to_vec())
        .build()
        .await;
    let admin = admin(&app).await;
    let shares = ShareState::from_ref(&app.state);

    let share = shares
        .service
        .create_share(
            &admin,
            CreateShareRequest {
                connection_id: "local".into(),
                path: "/shared.txt".into(),
                password: None,
                expires_in_hours: Some(24),
            },
        )
        .await
        .unwrap();

    let (connection, path) = shares
        .service
        .verify_and_get_public_share(&share.share_token, None)
        .await
        .unwrap();
    assert_eq!(connection, "local");
    assert_eq!(path, "/shared.txt");

    shares.service.delete_share(&admin, &share.id).await.unwrap();
    assert!(shares
        .service
        .verify_and_get_public_share(&share.share_token, None)
        .await
        .is_err());
}
