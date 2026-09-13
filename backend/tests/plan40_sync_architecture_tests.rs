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
use backend::events::{DomainEvent, EventJournal, ReplayOutcome};
use backend::middleware::REQUEST_ID_HEADER;
use backend::ports::transfer::TransferType;
use backend::state::{
    AppState, FileApiState, RealtimeState, RuntimeOwner, ShutdownReason, TransferState,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tempfile::tempdir;
use tower::ServiceExt;

struct TestRuntime {
    temp: tempfile::TempDir,
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
            temp,
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

    let names: Vec<String> = listing.entries.into_iter().map(|e| e.name).collect();
    assert!(names.contains(&"visible_file.txt".to_string()));
    assert!(
        !names.iter().any(|n| n.contains(".aerofs-part-")),
        "Staging files must be filtered out"
    );
}

#[tokio::test]
async fn test_websocket_replay_result_resync_required_on_expired_sequence() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("replay_retention.db");
    let database_url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    let db = init_db(&database_url).await.unwrap();
    let journal = Arc::new(EventJournal::init(db.clone()).await.unwrap());

    for i in 0..550 {
        journal
            .append(
                DomainEvent::file_change("local", format!("/file_{i}.txt"), "create"),
                None,
            )
            .await
            .unwrap();
    }

    sqlx::query("DELETE FROM event_journal WHERE sequence <= 50")
        .execute(&db)
        .await
        .unwrap();

    let replay_result = journal.get_since(Some(journal.epoch()), 1, 1000).await.unwrap();
    match replay_result {
        ReplayOutcome::Expired { latest_sequence } => assert!(latest_sequence >= 550),
        other => panic!("Expected Expired result for sequence 1, got {other:?}"),
    }

    let recent_result = journal.get_since(Some(journal.epoch()), 540, 1000).await.unwrap();
    match recent_result {
        ReplayOutcome::Events(events) => assert!(!events.is_empty(), "Should replay recent retained events"),
        other => panic!("Expected Events result for sequence 540, got {other:?}"),
    }
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
    let body1: Value = serde_json::from_slice(&to_bytes(response1.into_body(), usize::MAX).await.unwrap()).unwrap();
    let job_id1 = body1["job_id"].as_str().unwrap().to_string();

    let response2 = app
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
    assert_eq!(response2.status(), StatusCode::ACCEPTED);
    let body2: Value = serde_json::from_slice(&to_bytes(response2.into_body(), usize::MAX).await.unwrap()).unwrap();
    let job_id2 = body2["job_id"].as_str().unwrap().to_string();

    assert_eq!(job_id1, job_id2, "Submitting with same idempotency key must return existing job ID");
}

#[tokio::test]
async fn test_transfer_event_ordering_and_causality() {
    let (_app, state, _cookie, runtime) = setup_test_app().await;
    let admin = admin_user();

    let src_file = runtime.temp.path().join("storage").join("order_src.txt");
    std::fs::write(&src_file, b"ordering test").unwrap();

    let realtime = RealtimeState::from_ref(&state);
    let mut rx = realtime.service.subscribe();
    let transfers = TransferState::from_ref(&state);

    transfers
        .use_cases
        .create_transfer
        .execute(
            &actor(&admin),
            backend::application::transfers::CreateTransferCommand {
                name: "order_test".to_string(),
                transfer_type: TransferType::Copy,
                source_connection: ConnectionId::local(),
                source_path: "/order_src.txt".to_string(),
                destination_connection: ConnectionId::local(),
                destination_path: "/order_dst.txt".to_string(),
            },
        )
        .await
        .unwrap();

    let mut file_change_seq = None;
    let mut completed_seq = None;
    for _ in 0..50 {
        if let Ok(Ok(env)) = tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
            match env.event {
                DomainEvent::FileChange { path, action, .. }
                    if path == "/order_dst.txt" && action == "create" =>
                {
                    file_change_seq = Some(env.sequence);
                }
                DomainEvent::TransferCompleted(job)
                    if job.get("destination_path").and_then(|v| v.as_str()) == Some("/order_dst.txt") =>
                {
                    completed_seq = Some(env.sequence);
                    break;
                }
                _ => {}
            }
        }
    }

    assert!(file_change_seq.is_some(), "FileChange event must be emitted for destination");
    assert!(completed_seq.is_some(), "TransferCompleted event must be emitted");
    assert!(
        file_change_seq.unwrap() < completed_seq.unwrap(),
        "FileChange must have a lower sequence number than TransferCompleted"
    );
}
