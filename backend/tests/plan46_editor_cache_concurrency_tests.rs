use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use backend::{config::AppConfig, create_router, db::init_db, AppState};
use serde_json::json;
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

async fn login_admin(app: &axum::Router) -> String {
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

    cookie_header.split(';').next().unwrap().to_string()
}

#[tokio::test]
async fn test_file_content_cache_control_headers() {
    let (app, _temp) = setup_test_app().await;
    let session_cookie = login_admin(&app).await;

    // 1. Create a file
    let create_req = Request::builder()
        .uri("/api/v1/connections/local/files")
        .method("POST")
        .header(header::COOKIE, &session_cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "path": "/hello.txt" }).to_string()))
        .unwrap();

    let resp = app.clone().oneshot(create_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // 2. Put some content
    let update_req = Request::builder()
        .uri("/api/v1/connections/local/files/content")
        .method("PUT")
        .header(header::COOKIE, &session_cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "path": "/hello.txt", "content": "Hello World" }).to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(update_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 3. GET file content and check Cache-Control headers
    let get_req = Request::builder()
        .uri("/api/v1/connections/local/files/content?path=/hello.txt")
        .method("GET")
        .header(header::COOKIE, &session_cookie)
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(get_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let cache_control = resp
        .headers()
        .get(header::CACHE_CONTROL)
        .expect("Cache-Control header should be present")
        .to_str()
        .unwrap();

    // Verify it contains no-store and no-cache, and NOT max-age=3600
    assert!(
        cache_control.contains("no-store"),
        "Cache-Control should contain no-store, got: {}",
        cache_control
    );
    assert!(
        !cache_control.contains("max-age=3600"),
        "Cache-Control should NOT contain max-age=3600, got: {}",
        cache_control
    );

    // Verify ETag is present
    assert!(
        resp.headers().contains_key(header::ETAG),
        "ETag header must be returned"
    );
}

#[tokio::test]
async fn test_editor_edit_save_reopen_cycle() {
    let (app, _temp) = setup_test_app().await;
    let session_cookie = login_admin(&app).await;

    // 1. Create file with Version A
    let create_req = Request::builder()
        .uri("/api/v1/connections/local/files")
        .method("POST")
        .header(header::COOKIE, &session_cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "path": "/code.rs" }).to_string()))
        .unwrap();

    let resp = app.clone().oneshot(create_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let save_v1_req = Request::builder()
        .uri("/api/v1/connections/local/files/content")
        .method("PUT")
        .header(header::COOKIE, &session_cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "path": "/code.rs", "content": "fn main() { println!(\"Version A\"); }" })
                .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(save_v1_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let etag_v1 = resp
        .headers()
        .get(header::ETAG)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    // 2. Open in editor (GET) -> check initial content
    let get_v1_req = Request::builder()
        .uri("/api/v1/connections/local/files/content?path=/code.rs")
        .method("GET")
        .header(header::COOKIE, &session_cookie)
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(get_v1_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert_eq!(text, "fn main() { println!(\"Version A\"); }");

    // 3. User edits and saves Version B with If-Match: etag_v1
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    let version_b_content = "fn main() {\n    println!(\"Updated to Version B with new features\");\n}";
    let save_v2_req = Request::builder()
        .uri("/api/v1/connections/local/files/content")
        .method("PUT")
        .header(header::COOKIE, &session_cookie)
        .header(header::IF_MATCH, &etag_v1)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "path": "/code.rs", "content": version_b_content })
                .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(save_v2_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let etag_v2 = resp
        .headers()
        .get(header::ETAG)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert_ne!(etag_v1, etag_v2, "ETag must change after update");

    // 4. User closes editor and opens again (GET) -> MUST receive Version B and ETag B immediately
    let get_v2_req = Request::builder()
        .uri("/api/v1/connections/local/files/content?path=/code.rs")
        .method("GET")
        .header(header::COOKIE, &session_cookie)
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(get_v2_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let received_etag = resp
        .headers()
        .get(header::ETAG)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert_eq!(received_etag, etag_v2);
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert_eq!(text, version_b_content);
}

#[tokio::test]
async fn test_optimistic_concurrency_conflict_412() {
    let (app, temp) = setup_test_app().await;
    let session_cookie = login_admin(&app).await;

    // 1. Create file
    let create_req = Request::builder()
        .uri("/api/v1/connections/local/files")
        .method("POST")
        .header(header::COOKIE, &session_cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "path": "/conflict.txt" }).to_string()))
        .unwrap();

    let resp = app.clone().oneshot(create_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let put_req = Request::builder()
        .uri("/api/v1/connections/local/files/content")
        .method("PUT")
        .header(header::COOKIE, &session_cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "path": "/conflict.txt", "content": "Original Content" }).to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(put_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let original_etag = resp
        .headers()
        .get(header::ETAG)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    // 2. Simulate external modification directly on disk (changes mtime/size/etag)
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    let disk_file_path = temp.path().join("storage").join("conflict.txt");
    std::fs::write(&disk_file_path, "External Change on Disk").unwrap();

    // 3. User tries to save from editor using stale original_etag
    let stale_put_req = Request::builder()
        .uri("/api/v1/connections/local/files/content")
        .method("PUT")
        .header(header::COOKIE, &session_cookie)
        .header(header::IF_MATCH, &original_etag)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "path": "/conflict.txt", "content": "Editor Stale Update" }).to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(stale_put_req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::PRECONDITION_FAILED,
        "Should return 412 Precondition Failed when ETag differs"
    );
}

#[tokio::test]
async fn test_force_overwrite_header_bypasses_conflict() {
    let (app, temp) = setup_test_app().await;
    let session_cookie = login_admin(&app).await;

    // 1. Create file
    let create_req = Request::builder()
        .uri("/api/v1/connections/local/files")
        .method("POST")
        .header(header::COOKIE, &session_cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "path": "/force.txt" }).to_string()))
        .unwrap();

    let resp = app.clone().oneshot(create_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let put_req = Request::builder()
        .uri("/api/v1/connections/local/files/content")
        .method("PUT")
        .header(header::COOKIE, &session_cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "path": "/force.txt", "content": "Original Content" }).to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(put_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let stale_etag = resp
        .headers()
        .get(header::ETAG)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    // 2. Simulate external modification on disk
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    let disk_file_path = temp.path().join("storage").join("force.txt");
    std::fs::write(&disk_file_path, "External Change on Disk").unwrap();

    // 3. User clicks "Overwrite Disk" in UI -> sends X-Force-Overwrite: true
    let force_put_req = Request::builder()
        .uri("/api/v1/connections/local/files/content")
        .method("PUT")
        .header(header::COOKIE, &session_cookie)
        .header(header::IF_MATCH, &stale_etag)
        .header("X-Force-Overwrite", "true")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "path": "/force.txt", "content": "Forced Content Overwrite" }).to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(force_put_req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "X-Force-Overwrite: true should bypass precondition check and return 200 OK"
    );

    // 4. Verify disk content is now the forced content
    let content_on_disk = std::fs::read_to_string(&disk_file_path).unwrap();
    assert_eq!(content_on_disk, "Forced Content Overwrite");
}

#[tokio::test]
async fn test_wildcard_if_match_force_overwrite() {
    let (app, temp) = setup_test_app().await;
    let session_cookie = login_admin(&app).await;

    // 1. Create file
    let create_req = Request::builder()
        .uri("/api/v1/connections/local/files")
        .method("POST")
        .header(header::COOKIE, &session_cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "path": "/wildcard.txt" }).to_string()))
        .unwrap();

    let resp = app.clone().oneshot(create_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // 2. Modify disk directly
    let disk_file_path = temp.path().join("storage").join("wildcard.txt");
    std::fs::write(&disk_file_path, "External Disk Content").unwrap();

    // 3. Save with If-Match: * (wildcard)
    let wildcard_put_req = Request::builder()
        .uri("/api/v1/connections/local/files/content")
        .method("PUT")
        .header(header::COOKIE, &session_cookie)
        .header(header::IF_MATCH, "*")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "path": "/wildcard.txt", "content": "Wildcard Overwrite" }).to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(wildcard_put_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let content_on_disk = std::fs::read_to_string(&disk_file_path).unwrap();
    assert_eq!(content_on_disk, "Wildcard Overwrite");
}

#[tokio::test]
async fn test_cors_preflight_for_editor_headers() {
    let (app, _temp) = setup_test_app().await;

    let preflight_req = Request::builder()
        .uri("/api/v1/connections/local/files/content")
        .method("OPTIONS")
        .header("Origin", "http://localhost:5173")
        .header("Access-Control-Request-Method", "PUT")
        .header(
            "Access-Control-Request-Headers",
            "if-match, x-force-overwrite, content-type",
        )
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(preflight_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let allow_headers = resp
        .headers()
        .get("access-control-allow-headers")
        .expect("Access-Control-Allow-Headers must be present")
        .to_str()
        .unwrap()
        .to_lowercase();

    assert!(
        allow_headers.contains("if-match") || allow_headers.contains("*"),
        "Allow-Headers should contain if-match, got: {}",
        allow_headers
    );
    assert!(
        allow_headers.contains("x-force-overwrite") || allow_headers.contains("*"),
        "Allow-Headers should contain x-force-overwrite, got: {}",
        allow_headers
    );
}
