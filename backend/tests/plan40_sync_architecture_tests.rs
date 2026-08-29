use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use backend::auth::{AuthenticatedUser, UserInfo};
use backend::config::AppConfig;
use backend::create_router;
use backend::db::init_db;
use backend::middleware::REQUEST_ID_HEADER;
use backend::services::FileService;
use backend::transfer::{ReplayResult, WsEvent};
use backend::AppState;
use serde_json::{json, Value};
use tempfile::tempdir;
use tower::ServiceExt;

async fn setup_test_app() -> (axum::Router, AppState, String, tempfile::TempDir) {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("plan40_test.db");
    let storage_dir = temp.path().join("storage");
    std::fs::create_dir_all(&storage_dir).unwrap();

    let mut config = AppConfig::default();
    config.database.url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    config.filesystem.default_local_root = storage_dir;

    let db = init_db(&config.database.url).await.unwrap();
    let state = AppState::new_with_db(config, db).await;
    let app = create_router(state.clone());

    // Login as admin to get session cookie
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

    (app, state, cookie, temp)
}

#[tokio::test]
async fn test_request_id_middleware_propagation() {
    let (app, _state, _cookie, _temp) = setup_test_app().await;

    // 1. Without X-Request-ID (should auto-generate UUID)
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

    // 2. With client-supplied X-Request-ID (should echo back)
    let custom_id = "custom-trace-id-abc123xyz";
    let req2 = Request::builder()
        .uri("/api/v1/health/live")
        .method("GET")
        .header(REQUEST_ID_HEADER, custom_id)
        .body(Body::empty())
        .unwrap();

    let resp2 = app.oneshot(req2).await.unwrap();
    assert_eq!(resp2.status(), StatusCode::OK);
    let req_id2 = resp2
        .headers()
        .get(REQUEST_ID_HEADER)
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(req_id2, custom_id);
}

#[tokio::test]
async fn test_part_file_filtered_from_directory_listing() {
    let (_app, state, _cookie, _temp) = setup_test_app().await;
    let admin = AuthenticatedUser(UserInfo {
        id: "admin-id".to_string(),
        username: "admin".to_string(),
        is_admin: true,
    });

    // 1. Create a normal file and a .aerofs-part- staging file
    FileService::create_or_write_file(
        &state,
        &admin,
        "local",
        "/visible_file.txt",
        b"Visible".to_vec(),
        None,
    )
    .await
    .unwrap();

    FileService::create_or_write_file(
        &state,
        &admin,
        "local",
        "/.visible_file.txt.aerofs-part-job1234",
        b"Staging Part Data".to_vec(),
        None,
    )
    .await
    .unwrap();

    FileService::create_or_write_file(
        &state,
        &admin,
        "local",
        "/staging.dat.aerofs-part-job999",
        b"Staging Part Data 2".to_vec(),
        None,
    )
    .await
    .unwrap();

    // 2. List files with show_hidden = true
    let listing = FileService::list_directory(
        &state,
        &admin,
        "local",
        Some("/".into()),
        Some(true),
        None,
        None,
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
    let (_app, state, _cookie, _temp) = setup_test_app().await;

    // 1. Broadcast > 550 events to push history beyond its 500-capacity buffer
    for i in 0..550 {
        state.transfer_manager.broadcast_event(WsEvent::file_change(
            "local",
            format!("/file_{}.txt", i),
            "create",
        ));
    }

    // 2. Query sequence 1 (which has expired from the 500 buffer)
    let replay_result = state.transfer_manager.get_events_since(1).await;
    match replay_result {
        ReplayResult::Expired { latest_sequence } => {
            assert!(latest_sequence >= 550);
        }
        ReplayResult::Events(events) => {
            panic!(
                "Expected Expired result for sequence 1, got {} events",
                events.len()
            );
        }
    }

    // 3. Query a recent sequence (e.g. 540)
    let recent_result = state.transfer_manager.get_events_since(540).await;
    match recent_result {
        ReplayResult::Events(events) => {
            assert!(
                !events.is_empty(),
                "Should replay recent events within buffer"
            );
        }
        ReplayResult::Expired { .. } => {
            panic!("Recent sequence should not be expired");
        }
    }
}

#[tokio::test]
async fn test_transfer_idempotency_key_deduplication() {
    let (app, state, cookie, _temp) = setup_test_app().await;
    let admin = AuthenticatedUser(UserInfo {
        id: "admin-id".to_string(),
        username: "admin".to_string(),
        is_admin: true,
    });

    FileService::create_or_write_file(
        &state,
        &admin,
        "local",
        "/idemp_src.txt",
        b"Idempotency Test".to_vec(),
        None,
    )
    .await
    .unwrap();

    let idemp_key = "idemp-transfer-key-plan40-test";
    let payload = json!({
        "name": "idemp_job",
        "transfer_type": "copy",
        "source_connection_id": "local",
        "source_path": "/idemp_src.txt",
        "destination_connection_id": "local",
        "destination_path": "/idemp_dst.txt"
    });

    // 1. Initial request
    let req1 = Request::builder()
        .uri("/api/v1/transfers")
        .method("POST")
        .header(header::COOKIE, &cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .header("idempotency-key", idemp_key)
        .body(Body::from(payload.to_string()))
        .unwrap();

    let resp1 = app.clone().oneshot(req1).await.unwrap();
    assert_eq!(resp1.status(), StatusCode::ACCEPTED);
    let body1: Value =
        serde_json::from_slice(&to_bytes(resp1.into_body(), usize::MAX).await.unwrap()).unwrap();
    let job_id1 = body1["job_id"].as_str().unwrap().to_string();

    // 2. Retried request with identical Idempotency-Key
    let req2 = Request::builder()
        .uri("/api/v1/transfers")
        .method("POST")
        .header(header::COOKIE, &cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .header("idempotency-key", idemp_key)
        .body(Body::from(payload.to_string()))
        .unwrap();

    let resp2 = app.oneshot(req2).await.unwrap();
    assert_eq!(resp2.status(), StatusCode::ACCEPTED);
    assert_eq!(
        resp2
            .headers()
            .get("x-cache-idempotency")
            .map(|v| v.to_str().unwrap()),
        Some("HIT")
    );
    let body2: Value =
        serde_json::from_slice(&to_bytes(resp2.into_body(), usize::MAX).await.unwrap()).unwrap();
    let job_id2 = body2["job_id"].as_str().unwrap().to_string();

    assert_eq!(
        job_id1, job_id2,
        "Idempotent transfer submissions must return the identical job_id"
    );
}

#[tokio::test]
async fn test_event_ordering_file_change_before_transfer_completed() {
    let (_app, state, _cookie, _temp) = setup_test_app().await;
    let admin = AuthenticatedUser(UserInfo {
        id: "admin-id".to_string(),
        username: "admin".to_string(),
        is_admin: true,
    });

    FileService::create_or_write_file(
        &state,
        &admin,
        "local",
        "/order_src.txt",
        b"Ordering verification data".to_vec(),
        None,
    )
    .await
    .unwrap();

    let mut rx = state.transfer_manager.subscribe();

    backend::services::TransferService::create_transfer(
        &state,
        &admin,
        "order_job".into(),
        backend::transfer::TransferType::Copy,
        "local".into(),
        "/order_src.txt".into(),
        "local".into(),
        "/order_dst.txt".into(),
    )
    .await
    .unwrap();

    // Collect events until TransferCompleted
    let mut file_change_seq = None;
    let mut completed_seq = None;

    for _ in 0..50 {
        if let Ok(Ok(env)) =
            tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await
        {
            match env.event {
                WsEvent::FileChange { path, action, .. }
                    if path == "/order_dst.txt" && action == "create" =>
                {
                    file_change_seq = Some(env.sequence);
                }
                WsEvent::TransferCompleted(job) if job.destination_path == "/order_dst.txt" => {
                    completed_seq = Some(env.sequence);
                    break;
                }
                _ => {}
            }
        }
    }

    assert!(
        file_change_seq.is_some(),
        "FileChange event must be emitted for destination"
    );
    assert!(
        completed_seq.is_some(),
        "TransferCompleted event must be emitted"
    );
    assert!(
        file_change_seq.unwrap() < completed_seq.unwrap(),
        "FileChange must have a lower sequence number (emitted earlier) than TransferCompleted"
    );
}
