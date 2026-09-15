use axum::{
    body::{to_bytes, Body},
    extract::FromRef,
    http::{header, Request, StatusCode},
};
use backend::auth::{AuthenticatedUser, UserInfo};
use backend::bootstrap::build_application;
use backend::config::AppConfig;
use backend::create_router;
use backend::db::init_db;
use backend::domain::{Actor, ConnectionId};
use backend::middleware::REQUEST_ID_HEADER;
use backend::state::{AppState, FileApiState, RuntimeOwner, ShutdownReason};
use serde_json::{json, Value};
use tempfile::tempdir;
use tower::ServiceExt;

struct TestRuntime {
    _temp: tempfile::TempDir,
    runtime: RuntimeOwner,
}

impl Drop for TestRuntime {
    fn drop(&mut self) {
        self.runtime.request_shutdown(ShutdownReason::Manual);
    }
}

async fn setup_test_app() -> (axum::Router, AppState, String, TestRuntime) {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("plan40_test.db");
    let storage_dir = temp.path().join("storage");
    std::fs::create_dir_all(&storage_dir).unwrap();

    let mut config = AppConfig::default();
    config.database.url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    config.filesystem.default_local_root = storage_dir;

    let db = init_db(&config.database.url).await.unwrap();
    let built = build_application(config, db).await;
    let state = built.state;
    let app = create_router(state.clone());

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

    (
        app,
        state,
        cookie,
        TestRuntime {
            _temp: temp,
            runtime: built.runtime,
        },
    )
}

fn admin_user() -> AuthenticatedUser {
    AuthenticatedUser(UserInfo {
        id: "admin-id".to_string(),
        username: "admin".to_string(),
        is_admin: true,
    })
}

fn actor(user: &AuthenticatedUser) -> Actor {
    Actor {
        id: user.id.clone(),
        username: user.username.clone(),
        is_admin: user.is_admin,
    }
}

async fn write_file(
    state: &AppState,
    user: &AuthenticatedUser,
    path: &str,
    content: Vec<u8>,
) {
    let file_api = FileApiState::from_ref(state);
    file_api
        .files
        .write_file
        .execute(
            &actor(user),
            backend::application::files::WriteFileCommand {
                connection: ConnectionId::new("local").unwrap(),
                path: path.to_string(),
                content,
                expected_etag: None,
                create_only: false,
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn test_request_id_middleware_propagation() {
    let (app, _state, _cookie, _runtime) = setup_test_app().await;

    let req1 = Request::builder()
        .uri("/api/v1/health/live")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp1 = app.clone().oneshot(req1).await.unwrap();
    assert_eq!(resp1.status(), StatusCode::OK);
    let req_id1 = resp1.headers().get(REQUEST_ID_HEADER);
    assert!(req_id1.is_some(), "Response must include x-request-id");
    assert!(!req_id1.unwrap().to_str().unwrap().is_empty());

    let custom_id = "custom-trace-id-abc123xyz";
    let req2 = Request::builder()
        .uri("/api/v1/health/live")
        .method("GET")
        .header(REQUEST_ID_HEADER, custom_id)
        .body(Body::empty())
        .unwrap();
    let resp2 = app.oneshot(req2).await.unwrap();
    assert_eq!(resp2.status(), StatusCode::OK);
    let req_id2 = resp2.headers().get(REQUEST_ID_HEADER).unwrap().to_str().unwrap();
    assert_eq!(req_id2, custom_id);
}

#[tokio::test]
async fn test_part_file_filtered_from_directory_listing() {
    let (_app, state, _cookie, _runtime) = setup_test_app().await;
    let admin = admin_user();

    write_file(&state, &admin, "/visible_file.txt", b"Visible".to_vec()).await;
    write_file(
        &state,
        &admin,
        "/.visible_file.txt.aerofs-part-job1234",
        b"Staging Part Data".to_vec(),
    )
    .await;
    write_file(
        &state,
        &admin,
        "/staging.dat.aerofs-part-job999",
        b"Staging Part Data 2".to_vec(),
    )
    .await;

    let file_api = FileApiState::from_ref(&state);
    let listing = file_api
        .files
        .list_directory
        .execute(
            &actor(&admin),
            backend::application::files::ListDirectoryCommand {
                connection: ConnectionId::new("local").unwrap(),
                path: Some("/".into()),
                show_hidden: Some(true),
                sort: None,
                order: None,
                cursor: None,
                limit: None,
            },
        )
        .await
        .unwrap();

    let names: Vec<String> = listing.entries.into_iter().map(|entry| entry.name).collect();
    assert!(names.contains(&"visible_file.txt".to_string()));
    assert!(
        !names.iter().any(|name| name.contains(".aerofs-part-")),
        "Staging files must be filtered out"
    );
}

#[tokio::test]
async fn test_transfer_idempotency_key_deduplication() {
    let (app, _state, cookie, _runtime) = setup_test_app().await;
    let req_body = serde_json::json!({
        "name": "idempotent_test",
        "transfer_type": "copy",
        "source_connection_id": "local",
        "source_path": "/file_0.txt",
        "destination_connection_id": "local",
        "destination_path": "/file_0_idempotent.txt",
    });

    let response1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/transfers")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .header("Idempotency-Key", "test-key-123")
                .body(Body::from(serde_json::to_vec(&req_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response1.status(), StatusCode::ACCEPTED);
    let body1: Value = serde_json::from_slice(
        &to_bytes(response1.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let job_id1 = body1["job_id"].as_str().unwrap().to_string();

    let response2 = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/transfers")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .header("Idempotency-Key", "test-key-123")
                .body(Body::from(serde_json::to_vec(&req_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response2.status(), StatusCode::ACCEPTED);
    let body2: Value = serde_json::from_slice(
        &to_bytes(response2.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let job_id2 = body2["job_id"].as_str().unwrap().to_string();

    assert_eq!(
        job_id1, job_id2,
        "Submitting with same idempotency key must return existing job ID"
    );
}
