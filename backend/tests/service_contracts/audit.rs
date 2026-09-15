use axum::extract::FromRef;
use backend::{
    auth::{AuthenticatedUser, UserInfo},
    state::AuditState,
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
async fn recorded_audit_entry_is_queryable_by_admin() {
    let app = TestAppBuilder::new().running().build().await;
    let admin = admin(&app).await;
    let audit = AuditState::from_ref(&app.state);

    audit
        .service
        .record(
            Some(&admin.id),
            "TEST_AUDIT",
            Some("local"),
            Some("/audit-target.txt"),
            "SUCCESS",
            Some("127.0.0.1"),
            Some("audit contract"),
        )
        .await
        .unwrap();

    let logs = audit.service.list_logs(&admin, 10, 0).await.unwrap();
    assert!(logs.iter().any(|entry| entry.action == "TEST_AUDIT"));
}
