use axum::http::{Method, StatusCode};
use backend::{
    domain::{Actor, ConnectionId},
    infrastructure::files::SqliteAuthorization,
    ports::authorization::{Authorization, FileAction},
};
use chrono::Utc;
use serde_json::json;

use crate::support::{response_json, PermissionGrant, TestAppBuilder, TestAuth};

async fn seed_remote_connection(app: &crate::support::TestApp, id: &str, read_only: bool) {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO connections (id, name, provider, base_path, read_only, enabled, created_at, updated_at) VALUES (?, ?, 's3', '/', ?, 1, ?, ?)",
    )
    .bind(id)
    .bind(format!("Remote {id}"))
    .bind(i64::from(read_only))
    .bind(&now)
    .bind(&now)
    .execute(&app.db)
    .await
    .expect("insert remote fixture connection");
}

#[tokio::test]
async fn authorization_policy_distinguishes_local_fallback_remote_grants_and_read_only() {
    let app = TestAppBuilder::new().running().build().await;
    let regular = app.seed_session_user("policy-user", false).await;
    let authorization = SqliteAuthorization::new(app.db.clone());
    let actor = Actor {
        id: regular.id.clone(),
        username: regular.username.clone(),
        is_admin: false,
    };
    let local = ConnectionId::new("local").unwrap();

    authorization
        .authorize(&actor, &local, FileAction::Write)
        .await
        .expect("local storage keeps the documented default permission fallback");

    let remote = ConnectionId::new("remote-authz").unwrap();
    seed_remote_connection(&app, remote.as_str(), false).await;
    let denied = authorization
        .authorize(&actor, &remote, FileAction::Read)
        .await
        .expect_err("remote storage requires an explicit permission row");
    assert!(matches!(denied, backend::errors::AppError::Forbidden(_)));

    app.grant_permissions(&regular, remote.as_str(), PermissionGrant::read_only())
        .await;
    for action in [FileAction::List, FileAction::Read, FileAction::Download] {
        authorization
            .authorize(&actor, &remote, action)
            .await
            .expect("read-only grant should allow read-class operations");
    }
    for action in [
        FileAction::Create,
        FileAction::Write,
        FileAction::Upload,
        FileAction::Delete,
    ] {
        let denied = authorization
            .authorize(&actor, &remote, action)
            .await
            .expect_err("read-only grant must reject mutation-class operations");
        assert!(matches!(denied, backend::errors::AppError::Forbidden(_)));
    }

    app.grant_permissions(&regular, remote.as_str(), PermissionGrant::full())
        .await;
    authorization
        .authorize(&actor, &remote, FileAction::Write)
        .await
        .expect("full explicit grant permits mutation");

    let admin = Actor {
        id: "admin-fixture".to_string(),
        username: "admin".to_string(),
        is_admin: true,
    };
    authorization
        .authorize(&admin, &remote, FileAction::Write)
        .await
        .expect("administrator bypasses ordinary permission rows");

    sqlx::query("UPDATE connections SET read_only = 1 WHERE id = ?")
        .bind(remote.as_str())
        .execute(&app.db)
        .await
        .unwrap();
    let denied = authorization
        .authorize(&admin, &remote, FileAction::Write)
        .await
        .expect_err("connection read-only policy must constrain administrators too");
    assert!(matches!(denied, backend::errors::AppError::Forbidden(_)));
}

#[tokio::test]
async fn http_distinguishes_unauthenticated_from_authenticated_but_forbidden() {
    let app = TestAppBuilder::new().running().build().await;
    let payload = json!({
        "name": "Unauthorized Remote",
        "provider": "s3",
        "host": "example.invalid"
    });

    let anonymous = app
        .json_request(
            Method::POST,
            "/api/v1/connections",
            TestAuth::Anonymous,
            Some(payload.clone()),
        )
        .await;
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);
    let anonymous = response_json(anonymous).await;
    assert_eq!(anonymous["error"]["code"], "SESSION_EXPIRED");
    assert_eq!(anonymous["error"]["category"], "authentication");

    let regular = app.seed_session_user("regular-http", false).await;
    let session = app.session_for_user(&regular).await;
    let forbidden = app
        .json_request(
            Method::POST,
            "/api/v1/connections",
            TestAuth::Cookie(&session),
            Some(payload),
        )
        .await;
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);
    let forbidden = response_json(forbidden).await;
    assert_eq!(forbidden["error"]["code"], "FORBIDDEN");
    assert_eq!(forbidden["error"]["category"], "authorization");
}

#[tokio::test]
async fn authenticated_regular_user_can_use_documented_local_permission_fallback() {
    let app = TestAppBuilder::new()
        .running()
        .with_file("visible.txt", b"visible".to_vec())
        .build()
        .await;
    let user = app.seed_session_user("local-user", false).await;
    let session = app.session_for_user(&user).await;

    let response = app
        .json_request(
            Method::GET,
            "/api/v1/connections/local/files?path=/",
            TestAuth::Cookie(&session),
            None,
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert!(body["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["name"] == "visible.txt"));
}
