use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use backend::{config::AppConfig, create_router, db::{init_db, DbPool}, AppState};
use serde_json::{json, Value};
use tempfile::tempdir;
use tower::ServiceExt;

async fn setup_app() -> (axum::Router, String, tempfile::TempDir, DbPool) {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("share_auth_test.db");
    let storage_dir = temp.path().join("storage");
    std::fs::create_dir_all(&storage_dir).unwrap();
    std::fs::write(storage_dir.join("shared_file.txt"), b"Secret shared contents").unwrap();

    let mut config = AppConfig::default();
    config.database.url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    config.filesystem.default_local_root = storage_dir;

    let db = init_db(&config.database.url).await.unwrap();
    let state = AppState::new_with_db(config, db.clone()).await;
    let app = create_router(state);

    let login_req = Request::builder()
        .uri("/api/v1/auth/login")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "username": "admin", "password": "admin12345" }).to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(login_req).await.unwrap();
    let cookie_header = resp.headers().get(header::SET_COOKIE).unwrap().to_str().unwrap();
    let cookie = cookie_header.split(';').next().unwrap().to_string();

    (app, cookie, temp, db)
}

#[tokio::test]
async fn test_share_password_verification_and_protection() {
    let (app, cookie, _temp, _db) = setup_app().await;
    let create_share_req = Request::builder()
        .uri("/api/v1/shares")
        .method("POST")
        .header(header::COOKIE, &cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "connection_id": "local",
                "path": "/shared_file.txt",
                "password": "mypassword123",
                "expires_in_hours": 24
            }).to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(create_share_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let share_res: Value = serde_json::from_slice(&body).unwrap();
    let token = share_res["share_token"].as_str().unwrap();
    assert!(share_res["has_password"].as_bool().unwrap());

    let resp = app.clone().oneshot(Request::builder()
        .uri(format!("/api/v1/shares/public/{token}"))
        .method("GET").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let resp = app.clone().oneshot(Request::builder()
        .uri(format!("/api/v1/shares/public/{token}?password=wrongpassword"))
        .method("GET").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let resp = app.clone().oneshot(Request::builder()
        .uri(format!("/api/v1/shares/public/{token}?password=mypassword123"))
        .method("GET").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let content_bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    assert_eq!(content_bytes, b"Secret shared contents".as_slice());
}

#[tokio::test]
async fn test_websocket_authentication_enforcement() {
    let (app, _cookie, _temp, _db) = setup_app().await;
    let ws_req = Request::builder()
        .uri("/api/v1/ws")
        .method("GET")
        .header(header::UPGRADE, "websocket")
        .header(header::CONNECTION, "Upgrade")
        .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
        .header("Sec-WebSocket-Version", "13")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(ws_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_multiuser_permission_enforcement() {
    let (app, _admin_cookie, _temp, db) = setup_app().await;
    let user_id = uuid::Uuid::new_v4().to_string();
    let user_pwd_hash = backend::auth::hash_password("userpass123").unwrap();
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO users (id, username, password_hash, is_admin, created_at, updated_at) VALUES (?, 'regular_user', ?, 0, ?, ?)"
    )
    .bind(&user_id).bind(&user_pwd_hash).bind(&now).bind(&now)
    .execute(&db).await.unwrap();

    let login_req = Request::builder()
        .uri("/api/v1/auth/login")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "username": "regular_user", "password": "userpass123" }).to_string()))
        .unwrap();
    let resp = app.clone().oneshot(login_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let user_cookie = resp.headers().get(header::SET_COOKIE).unwrap().to_str().unwrap()
        .split(';').next().unwrap().to_string();

    let list_req = Request::builder()
        .uri("/api/v1/connections/local/files?path=/")
        .method("GET")
        .header(header::COOKIE, &user_cookie)
        .body(Body::empty()).unwrap();
    let resp = app.clone().oneshot(list_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}
