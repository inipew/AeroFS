use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use backend::{config::AppConfig, create_router, db::init_db, AppState};
use serde_json::{json, Value};
use tempfile::tempdir;
use tower::ServiceExt;

#[tokio::test]
async fn upload_session_admits_streams_and_completes_the_same_transfer_job() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("upload_session.db");
    let storage = temp.path().join("storage");
    std::fs::create_dir_all(&storage).unwrap();
    let mut config = AppConfig::default();
    config.database.url = format!("sqlite://{}?mode=rwc", db_path.display());
    config.filesystem.default_local_root = storage.clone();
    let state = AppState::new_with_db(config, init_db(&format!("sqlite://{}?mode=rwc", db_path.display())).await.unwrap()).await;
    let app = create_router(state);

    let login = Request::builder()
        .uri("/api/v1/auth/login").method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"username":"admin","password":"admin12345"}).to_string())).unwrap();
    let login = app.clone().oneshot(login).await.unwrap();
    let cookie = login.headers().get(header::SET_COOKIE).unwrap().to_str().unwrap().split(';').next().unwrap().to_string();

    let admit = Request::builder()
        .uri("/api/v1/connections/local/uploads").method("POST")
        .header(header::COOKIE, &cookie).header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"path":"/session.txt","file_name":"session.txt","total_bytes":5}).to_string())).unwrap();
    let admit = app.clone().oneshot(admit).await.unwrap();
    assert_eq!(admit.status(), StatusCode::ACCEPTED);
    let admitted: Value = serde_json::from_slice(&to_bytes(admit.into_body(), usize::MAX).await.unwrap()).unwrap();
    let job_id = admitted["job_id"].as_str().unwrap();
    let upload_url = admitted["upload_url"].as_str().unwrap();

    let stream = Request::builder()
        .uri(upload_url).method("PUT").header(header::COOKIE, &cookie)
        .header(header::CONTENT_TYPE, "text/plain").body(Body::from("hello")).unwrap();
    let stream = app.clone().oneshot(stream).await.unwrap();
    assert_eq!(stream.status(), StatusCode::OK);

    let jobs = Request::builder().uri("/api/v1/transfers").method("GET")
        .header(header::COOKIE, &cookie).body(Body::empty()).unwrap();
    let jobs = app.oneshot(jobs).await.unwrap();
    let jobs: Value = serde_json::from_slice(&to_bytes(jobs.into_body(), usize::MAX).await.unwrap()).unwrap();
    let job = jobs.as_array().unwrap().iter().find(|job| job["id"] == job_id).unwrap();
    assert_eq!(job["status"], "completed");
    assert_eq!(std::fs::read(storage.join("session.txt")).unwrap(), b"hello");
}
