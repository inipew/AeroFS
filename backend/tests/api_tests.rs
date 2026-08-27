use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use backend::{config::AppConfig, create_router, db::init_db, AppState};
use serde_json::{json, Value};
use tempfile::tempdir;
use tower::ServiceExt;

async fn setup_test_app() -> (axum::Router, tempfile::TempDir) {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("test.db");
    let storage_dir = temp.path().join("storage");
    std::fs::create_dir_all(&storage_dir).unwrap();

    let mut config = AppConfig::default();
    config.database.url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    config.filesystem.default_local_root = storage_dir;

    let db = init_db(&config.database.url).await.unwrap();
    let state = AppState::new_with_db(config, db).await;
    let app = create_router(state);

    (app, temp)
}

#[tokio::test]
async fn test_auth_and_file_api_flow() {
    let (app, _temp) = setup_test_app().await;

    // 1. Test Login with wrong credentials -> 401
    let login_req = Request::builder()
        .uri("/api/v1/auth/login")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "username": "admin", "password": "wrongpassword" }).to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(login_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // 2. Test Login with correct credentials -> 200 + Set-Cookie
    let login_req = Request::builder()
        .uri("/api/v1/auth/login")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "username": "admin", "password": "admin12345" }).to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(login_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let cookie_header = resp
        .headers()
        .get(header::SET_COOKIE)
        .expect("Set-Cookie header should be present")
        .to_str()
        .unwrap()
        .to_string();
    assert!(cookie_header.contains("session_id="));

    let session_cookie = cookie_header.split(';').next().unwrap();

    // 3. Test /auth/me with Cookie -> 200 User Info
    let me_req = Request::builder()
        .uri("/api/v1/auth/me")
        .method("GET")
        .header(header::COOKIE, session_cookie)
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(me_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let user_val: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(user_val["username"], "admin");
    assert_eq!(user_val["is_admin"], true);

    // 4. Test List Connections -> 200
    let conn_req = Request::builder()
        .uri("/api/v1/connections")
        .method("GET")
        .header(header::COOKIE, session_cookie)
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(conn_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 5. Test Create Directory -> 201
    let mkdir_req = Request::builder()
        .uri("/api/v1/connections/local/directories")
        .method("POST")
        .header(header::COOKIE, session_cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "path": "/projects" }).to_string()))
        .unwrap();

    let resp = app.clone().oneshot(mkdir_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // 6. Test Create File -> 201
    let mkfile_req = Request::builder()
        .uri("/api/v1/connections/local/files")
        .method("POST")
        .header(header::COOKIE, session_cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "path": "/projects/notes.txt" }).to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(mkfile_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // 7. Test List Files in /projects -> 200
    let list_req = Request::builder()
        .uri("/api/v1/connections/local/files?path=/projects")
        .method("GET")
        .header(header::COOKIE, session_cookie)
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(list_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let listing: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(listing["total_count"], 1);
    assert_eq!(listing["entries"][0]["name"], "notes.txt");

    // 8. Test Delete File -> 200
    let del_req = Request::builder()
        .uri("/api/v1/connections/local/files")
        .method("DELETE")
        .header(header::COOKIE, session_cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "paths": ["/projects/notes.txt"] }).to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(del_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_embedded_static_assets_and_spa_fallback() {
    let (app, _temp) = setup_test_app().await;

    // 1. Test root "/" -> returns index.html with 200 OK and text/html
    let req = Request::builder()
        .uri("/")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let content_type = resp.headers().get(header::CONTENT_TYPE).unwrap().to_str().unwrap();
    assert!(content_type.contains("text/html"));
    let body = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let body_str = String::from_utf8_lossy(&body);
    assert!(body_str.contains("<!DOCTYPE html>") || body_str.contains("<html") || body_str.contains("AeroFS") || body_str.contains("id=\"app\""));

    // 2. Test SPA fallback route "/browse/some/deep/folder" -> returns index.html with 200 OK
    let req = Request::builder()
        .uri("/browse/some/deep/folder")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let content_type = resp.headers().get(header::CONTENT_TYPE).unwrap().to_str().unwrap();
    assert!(content_type.contains("text/html"));
}

#[tokio::test]
async fn test_editor_save_preserves_destination_permissions() {
    let (app, _temp) = setup_test_app().await;

    // Login as admin
    let login_req = Request::builder()
        .uri("/api/v1/auth/login")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "username": "admin", "password": "admin12345" }).to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(login_req).await.unwrap();
    let cookie_header = resp.headers().get(header::SET_COOKIE).unwrap().to_str().unwrap().to_string();
    let session_cookie = cookie_header.split(';').next().unwrap();

    // 1. Create file
    let create_req = Request::builder()
        .uri("/api/v1/connections/local/files")
        .method("POST")
        .header(header::COOKIE, session_cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "path": "/secure.conf" }).to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(create_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // 2. Chmod file to 0600 (octal 384)
    #[cfg(unix)]
    {
        let chmod_req = Request::builder()
            .uri("/api/v1/connections/local/files/chmod")
            .method("POST")
            .header(header::COOKIE, session_cookie)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({ "path": "/secure.conf", "mode": 0o600 }).to_string(),
            ))
            .unwrap();

        let resp = app.clone().oneshot(chmod_req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Verify stat returns "0600"
        let stat_req = Request::builder()
            .uri("/api/v1/connections/local/files/metadata?path=/secure.conf")
            .method("GET")
            .header(header::COOKIE, session_cookie)
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(stat_req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let meta_val: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(meta_val["permissions"], "0600");

        // 3. Save modified content via editor API
        let update_req = Request::builder()
            .uri("/api/v1/connections/local/files/content")
            .method("PUT")
            .header(header::COOKIE, session_cookie)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({
                    "path": "/secure.conf",
                    "content": "SECRET_KEY=ABCDEF123456"
                }).to_string(),
            ))
            .unwrap();

        let resp = app.clone().oneshot(update_req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // 4. Verify stat still reports exact "0600" (preserved after atomic save!)
        let stat_req2 = Request::builder()
            .uri("/api/v1/connections/local/files/metadata?path=/secure.conf")
            .method("GET")
            .header(header::COOKIE, session_cookie)
            .body(Body::empty())
            .unwrap();

        let resp2 = app.clone().oneshot(stat_req2).await.unwrap();
        assert_eq!(resp2.status(), StatusCode::OK);
        let body2 = to_bytes(resp2.into_body(), usize::MAX).await.unwrap();
        let meta_val2: Value = serde_json::from_slice(&body2).unwrap();
        assert_eq!(meta_val2["permissions"], "0600");
    }
}
