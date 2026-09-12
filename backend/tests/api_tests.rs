use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use backend::{bootstrap::build_application, config::AppConfig, create_router, db::init_db, state::RuntimePhase};
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
    let built = build_application(config, db).await;
    built.runtime.set_phase(RuntimePhase::Running);
    let app = create_router(built.state);

    (app, temp)
}

#[tokio::test]
async fn test_auth_and_file_api_flow() {
    let (app, _temp) = setup_test_app().await;

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

    let conn_req = Request::builder()
        .uri("/api/v1/connections")
        .method("GET")
        .header(header::COOKIE, session_cookie)
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(conn_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let mkdir_req = Request::builder()
        .uri("/api/v1/connections/local/directories")
        .method("POST")
        .header(header::COOKIE, session_cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "path": "/projects" }).to_string()))
        .unwrap();

    let resp = app.clone().oneshot(mkdir_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

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

    let duplicate_file_req = Request::builder()
        .uri("/api/v1/connections/local/files")
        .method("POST")
        .header(header::COOKIE, session_cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "path": "/projects/notes.txt" }).to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(duplicate_file_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let duplicate_error: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(duplicate_error["error"]["code"], "ALREADY_EXISTS");
    assert_eq!(duplicate_error["error"]["category"], "conflict");

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

    let req = Request::builder()
        .uri("/")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let content_type = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(content_type.contains("text/html"));
    let body = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let body_str = String::from_utf8_lossy(&body);
    assert!(
        body_str.contains("<!DOCTYPE html>")
            || body_str.contains("<html")
            || body_str.contains("AeroFS")
            || body_str.contains("id=\"app\"")
    );

    let req = Request::builder()
        .uri("/browse/some/deep/folder")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let content_type = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(content_type.contains("text/html"));
}

#[tokio::test]
async fn test_editor_save_preserves_destination_permissions() {
    let (app, _temp) = setup_test_app().await;

    let login_req = Request::builder()
        .uri("/api/v1/auth/login")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "username": "admin", "password": "admin12345" }).to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(login_req).await.unwrap();
    let cookie_header = resp
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let session_cookie = cookie_header.split(';').next().unwrap();

    let create_req = Request::builder()
        .uri("/api/v1/connections/local/files")
        .method("POST")
        .header(header::COOKIE, session_cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "path": "/secure.conf" }).to_string()))
        .unwrap();

    let resp = app.clone().oneshot(create_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

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

        let update_req = Request::builder()
            .uri("/api/v1/connections/local/files/content")
            .method("PUT")
            .header(header::COOKIE, session_cookie)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({
                    "path": "/secure.conf",
                    "content": "SECRET_KEY=ABCDEF123456"
                })
                .to_string(),
            ))
            .unwrap();

        let resp = app.clone().oneshot(update_req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

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

#[tokio::test]
async fn test_max_editable_size_enforcement() {
    let (app, _temp) = setup_test_app().await;

    let login_req = Request::builder()
        .uri("/api/v1/auth/login")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "username": "admin", "password": "admin12345" }).to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(login_req).await.unwrap();
    let cookie_header = resp
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let session_cookie = cookie_header.split(';').next().unwrap();

    let create_req = Request::builder()
        .uri("/api/v1/connections/local/files")
        .method("POST")
        .header(header::COOKIE, session_cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "path": "/big.txt" }).to_string()))
        .unwrap();

    let resp = app.clone().oneshot(create_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let huge_content = "A".repeat(15 * 1024 * 1024);
    let update_req = Request::builder()
        .uri("/api/v1/connections/local/files/content")
        .method("PUT")
        .header(header::COOKIE, session_cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "path": "/big.txt",
                "content": huge_content
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(update_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn test_idempotency_key_deduplication() {
    let (app, _temp) = setup_test_app().await;

    let login_req = Request::builder()
        .uri("/api/v1/auth/login")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "username": "admin", "password": "admin12345" }).to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(login_req).await.unwrap();
    let cookie_header = resp
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let session_cookie = cookie_header.split(';').next().unwrap();

    let idempotency_key = "idemp-key-create-dir-12345";

    let create_dir_req = Request::builder()
        .uri("/api/v1/connections/local/directories")
        .method("POST")
        .header(header::COOKIE, session_cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .header("idempotency-key", idempotency_key)
        .body(Body::from(
            json!({ "path": "/idempotent_folder" }).to_string(),
        ))
        .unwrap();

    let resp1 = app.clone().oneshot(create_dir_req).await.unwrap();
    assert_eq!(resp1.status(), StatusCode::CREATED);

    let duplicate_req = Request::builder()
        .uri("/api/v1/connections/local/directories")
        .method("POST")
        .header(header::COOKIE, session_cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .header("idempotency-key", idempotency_key)
        .body(Body::from(
            json!({ "path": "/idempotent_folder" }).to_string(),
        ))
        .unwrap();

    let resp2 = app.clone().oneshot(duplicate_req).await.unwrap();
    assert_eq!(resp2.status(), StatusCode::CREATED);
    assert_eq!(
        resp2
            .headers()
            .get("x-cache-idempotency")
            .unwrap()
            .to_str()
            .unwrap(),
        "HIT"
    );
}

#[tokio::test]
async fn test_health_live_and_ready_endpoints() {
    let (app, _temp) = setup_test_app().await;

    let live_req = Request::builder()
        .uri("/health/live")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(live_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let val: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(val["status"], "alive");

    let ready_req = Request::builder()
        .uri("/health/ready")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(ready_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let val: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(val["status"], "ready");
    assert_eq!(val["database"], "connected");
}

#[tokio::test]
async fn test_preview_security_headers_isolation() {
    let (app, _temp) = setup_test_app().await;

    let login_req = Request::builder()
        .uri("/api/v1/auth/login")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "username": "admin", "password": "admin12345" }).to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(login_req).await.unwrap();
    let cookie_header = resp
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let session_cookie = cookie_header.split(';').next().unwrap();

    let create_req = Request::builder()
        .uri("/api/v1/connections/local/files")
        .method("POST")
        .header(header::COOKIE, session_cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "path": "/vector.svg" }).to_string()))
        .unwrap();

    let resp = app.clone().oneshot(create_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let update_req = Request::builder()
        .uri("/api/v1/connections/local/files/content")
        .method("PUT")
        .header(header::COOKIE, session_cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "path": "/vector.svg",
                "content": "<svg><script>alert(1)</script></svg>"
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(update_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let get_req = Request::builder()
        .uri("/api/v1/connections/local/files/content?path=/vector.svg")
        .method("GET")
        .header(header::COOKIE, session_cookie)
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(get_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-security-policy")
            .unwrap()
            .to_str()
            .unwrap(),
        "default-src 'none'; sandbox"
    );
    assert_eq!(
        resp.headers()
            .get("x-content-type-options")
            .unwrap()
            .to_str()
            .unwrap(),
        "nosniff"
    );
}
