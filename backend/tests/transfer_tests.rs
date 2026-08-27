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
    let db_path = temp.path().join("transfer_test.db");
    let storage_dir = temp.path().join("storage");
    std::fs::create_dir_all(&storage_dir).unwrap();

    // Create a dummy source file
    let source_file = storage_dir.join("source.txt");
    std::fs::write(&source_file, b"Hello World from Background Transfer Engine!").unwrap();

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
async fn test_transfer_engine_queue_and_execution() {
    let (app, cookie, _temp) = setup_app().await;

    // 1. Submit a copy transfer job
    let transfer_req = Request::builder()
        .uri("/api/v1/transfers")
        .method("POST")
        .header(header::COOKIE, &cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "name": "Copy source.txt to destination.txt",
                "transfer_type": "copy",
                "source_connection_id": "local",
                "source_path": "/source.txt",
                "destination_connection_id": "local",
                "destination_path": "/destination.txt"
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(transfer_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let created: Value = serde_json::from_slice(&body).unwrap();
    let job_id = created["job_id"].as_str().unwrap().to_string();
    assert!(!job_id.is_empty());

    // Give background worker a brief moment to process
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

    // 2. List transfers
    let list_req = Request::builder()
        .uri("/api/v1/transfers")
        .method("GET")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(list_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let jobs: Value = serde_json::from_slice(&body).unwrap();
    let jobs_arr = jobs.as_array().unwrap();
    assert_eq!(jobs_arr.len(), 1);
    assert_eq!(jobs_arr[0]["id"], job_id);
    assert_eq!(jobs_arr[0]["status"], "completed");
    assert!(jobs_arr[0]["transferred_bytes"].as_u64().unwrap() > 0);
}
