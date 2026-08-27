use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use backend::{config::AppConfig, create_router, db::init_db, AppState};
use serde_json::{json, Value};
use tempfile::tempdir;
use tower::ServiceExt;

async fn setup_app() -> (axum::Router, String, tempfile::TempDir, AppState) {
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
    let app = create_router(state.clone());

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

    (app, cookie, temp, state)
}

#[tokio::test]
async fn test_transfer_engine_queue_and_execution() {
    let (app, cookie, _temp, _state) = setup_app().await;

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

    // Give background worker a moment to process
    tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;

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

#[tokio::test]
async fn test_transfer_cancellation_state_machine() {
    let (app, cookie, _temp, state) = setup_app().await;

    // 1. Submit a transfer job
    let job_id = state
        .transfer_manager
        .submit_job(
            "Cancel Me Job".into(),
            backend::transfer::TransferType::Copy,
            "local".into(),
            "/source.txt".into(),
            "local".into(),
            "/cancel_dest.txt".into(),
        )
        .await
        .unwrap();

    // 2. Immediately cancel the job
    let cancelled = state.transfer_manager.cancel_job(&job_id).await;
    assert!(cancelled);

    // Give background worker time to settle
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

    // 3. Verify status remains 'cancelled' (NEVER overwritten to 'completed')
    let list_req = Request::builder()
        .uri("/api/v1/transfers")
        .method("GET")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(list_req).await.unwrap();
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let jobs: Value = serde_json::from_slice(&body).unwrap();
    let job = jobs
        .as_array()
        .unwrap()
        .iter()
        .find(|j| j["id"] == job_id)
        .unwrap();
    assert_eq!(job["status"], "cancelled");
}

#[tokio::test]
async fn test_transfer_safe_move_semantics() {
    let (app, cookie, temp, _state) = setup_app().await;
    let storage_dir = temp.path().join("storage");
    let move_src = storage_dir.join("to_move.txt");
    let move_dst = storage_dir.join("moved.txt");
    std::fs::write(&move_src, b"Move Me Transactionally!").unwrap();

    // 1. Submit Move job
    let transfer_req = Request::builder()
        .uri("/api/v1/transfers")
        .method("POST")
        .header(header::COOKIE, &cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "name": "Move to_move.txt to moved.txt",
                "transfer_type": "move",
                "source_connection_id": "local",
                "source_path": "/to_move.txt",
                "destination_connection_id": "local",
                "destination_path": "/moved.txt"
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(transfer_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    // Give worker time to complete
    tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;

    // Verify source is deleted and destination exists
    assert!(!move_src.exists(), "Source file should be safely deleted after move");
    assert!(move_dst.exists(), "Destination file must exist after move");
    let content = std::fs::read(&move_dst).unwrap();
    assert_eq!(content, b"Move Me Transactionally!");
}

#[tokio::test]
async fn test_transfer_sqlite_durability() {
    let (_app, _cookie, _temp, state) = setup_app().await;

    // 1. Submit job to transfer manager
    let job_id = state
        .transfer_manager
        .submit_job(
            "Durable Job".into(),
            backend::transfer::TransferType::Copy,
            "local".into(),
            "/source.txt".into(),
            "local".into(),
            "/durable_dest.txt".into(),
        )
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;

    // 2. Query SQLite directly to verify durability
    let row: (String, String, String) = sqlx::query_as(
        "SELECT id, status, name FROM transfer_jobs WHERE id = ?"
    )
    .bind(&job_id)
    .fetch_one(&state.db)
    .await
    .unwrap();

    assert_eq!(row.0, job_id);
    assert_eq!(row.1, "completed");
    assert_eq!(row.2, "Durable Job");
}
