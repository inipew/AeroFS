use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use backend::{config::AppConfig, create_router, db::init_db, AppState};
use serde_json::{json, Value};
use tempfile::tempdir;
use tower::ServiceExt;

async fn setup_app() -> (axum::Router, String, tempfile::TempDir) {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("archive_test.db");
    let storage_dir = temp.path().join("storage");
    std::fs::create_dir_all(&storage_dir).unwrap();

    // Create some nested test files for search and archive
    let sub = storage_dir.join("documents");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("report.txt"), b"Quarterly Financial Report").unwrap();
    std::fs::write(sub.join("notes.md"), b"# Markdown Notes").unwrap();
    std::fs::write(storage_dir.join("root.txt"), b"Root Level File").unwrap();

    let mut config = AppConfig::default();
    config.database.url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    config.filesystem.default_local_root = storage_dir;

    let db = init_db(&config.database.url).await.unwrap();
    let state = AppState::new_with_db(config, db).await;
    let app = create_router(state);

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
    let cookie_header = resp
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let cookie = cookie_header.split(';').next().unwrap().to_string();

    (app, cookie, temp)
}

#[tokio::test]
async fn test_archive_compress_extract_and_search() {
    let (app, cookie, _temp) = setup_app().await;

    // 1. Recursive Search for "report" -> must find /documents/report.txt
    let search_req = Request::builder()
        .uri("/api/v1/connections/local/search?query=report")
        .method("GET")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(search_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let res_obj: Value = serde_json::from_slice(&body).unwrap();
    let results_arr = res_obj["results"].as_array().unwrap();
    assert_eq!(results_arr.len(), 1);
    assert_eq!(results_arr[0]["name"], "report.txt");

    // 2. Compress files into a ZIP archive
    let compress_req = Request::builder()
        .uri("/api/v1/connections/local/archive/compress")
        .method("POST")
        .header(header::COOKIE, &cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "base_path": "/",
                "relative_paths": ["/documents/report.txt", "/root.txt"],
                "destination_file": "/backup.zip",
                "format": "zip"
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(compress_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // 3. Extract the ZIP archive into /extracted
    let extract_req = Request::builder()
        .uri("/api/v1/connections/local/archive/extract")
        .method("POST")
        .header(header::COOKIE, &cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "archive_path": "/backup.zip",
                "destination_dir": "/extracted",
                "format": "zip"
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(extract_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 3b. Virtual Archive Browsing (plan/17.md) - list entries without full extraction
    let entries_req = Request::builder()
        .uri("/api/v1/connections/local/archive/entries?archive_path=/backup.zip")
        .method("GET")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(entries_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let entries_json: Value = serde_json::from_slice(&body).unwrap();
    let entries_arr = entries_json.as_array().unwrap();
    assert!(entries_arr
        .iter()
        .any(|e| e["name"] == "documents" && e["kind"] == "directory"));
    assert!(entries_arr
        .iter()
        .any(|e| e["name"] == "root.txt" && e["kind"] == "file"));

    // 3c. Virtual Archive Single Entry Read / Stream (plan/17.md)
    let read_req = Request::builder()
        .uri("/api/v1/connections/local/archive/read?archive_path=/backup.zip&entry_path=root.txt")
        .method("GET")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(read_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    assert_eq!(body.as_ref(), b"Root Level File");

    // 3d. Virtual Archive Selective Extract (plan/17.md)
    let sel_extract_req = Request::builder()
        .uri("/api/v1/connections/local/archive/extract-selected")
        .method("POST")
        .header(header::COOKIE, &cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "archive_path": "/backup.zip",
                "destination_dir": "/selective_extracted",
                "entries": ["root.txt"]
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(sel_extract_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 4. Verify extracted files via Search
    let search_ext_req = Request::builder()
        .uri("/api/v1/connections/local/search?path=/extracted&query=report")
        .method("GET")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(search_ext_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let res_obj: Value = serde_json::from_slice(&body).unwrap();
    let results_arr = res_obj["results"].as_array().unwrap();
    assert!(!results_arr.is_empty());

    // 5. Query Audit Logs (Admin only)
    let audit_req = Request::builder()
        .uri("/api/v1/audit-logs")
        .method("GET")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(audit_req).await.unwrap();
    let status = resp.status();
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    if status != StatusCode::OK {
        eprintln!(
            "Audit log response error: {}",
            String::from_utf8_lossy(&body)
        );
    }
    assert_eq!(status, StatusCode::OK);
    let logs: Value = serde_json::from_slice(&body).unwrap();
    let logs_arr = logs.as_array().unwrap();
    assert!(logs_arr.iter().any(|l| l["action"] == "ARCHIVE_COMPRESS"));
    assert!(logs_arr.iter().any(|l| l["action"] == "ARCHIVE_EXTRACT"));
    assert!(logs_arr
        .iter()
        .any(|l| l["action"] == "ARCHIVE_EXTRACT_SELECTED"));
}

#[tokio::test]
async fn test_archive_overwrite_modes() {
    let (app, cookie, temp) = setup_app().await;
    let storage_dir = temp.path().join("storage");

    // 1. Create a zip archive containing collision.txt
    let col_file = storage_dir.join("collision.txt");
    std::fs::write(&col_file, b"Original Content in Zip").unwrap();

    let compress_req = Request::builder()
        .uri("/api/v1/connections/local/archive/compress")
        .method("POST")
        .header(header::COOKIE, &cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "base_path": "/",
                "relative_paths": ["/collision.txt"],
                "destination_file": "/collision_test.zip",
                "format": "zip"
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(compress_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // 2. Pre-create target file in /target_dir with different content
    let target_dir = storage_dir.join("target_dir");
    std::fs::create_dir_all(&target_dir).unwrap();
    std::fs::write(
        target_dir.join("collision.txt"),
        b"Existing Content in Target",
    )
    .unwrap();

    // 3. Test overwrite_mode = "skip"
    let extract_skip_req = Request::builder()
        .uri("/api/v1/connections/local/archive/extract")
        .method("POST")
        .header(header::COOKIE, &cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "archive_path": "/collision_test.zip",
                "destination_dir": "/target_dir",
                "format": "zip",
                "overwrite_mode": "skip"
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(extract_skip_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let res_val: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(res_val["skipped_count"], 1);
    // Content should NOT be overwritten
    assert_eq!(
        std::fs::read(target_dir.join("collision.txt")).unwrap(),
        b"Existing Content in Target"
    );

    // 4. Test overwrite_mode = "keep_both"
    let extract_keep_req = Request::builder()
        .uri("/api/v1/connections/local/archive/extract")
        .method("POST")
        .header(header::COOKIE, &cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "archive_path": "/collision_test.zip",
                "destination_dir": "/target_dir",
                "format": "zip",
                "overwrite_mode": "keep_both"
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(extract_keep_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let res_val: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(res_val["entries_count"], 1);
    // collision (1).txt should now exist alongside collision.txt
    assert!(target_dir.join("collision (1).txt").is_file());
    assert_eq!(
        std::fs::read(target_dir.join("collision (1).txt")).unwrap(),
        b"Original Content in Zip"
    );
}
