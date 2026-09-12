use axum::extract::FromRef;
use backend::bootstrap::build_application;
use backend::cli::daemon_lock::DaemonLock;
use backend::config::AppConfig;
use backend::create_router;
use backend::db::init_db;
use backend::state::{RuntimeOwner, TransferState};
use backend::AppState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;
use tokio::net::TcpListener;

async fn setup_test_context() -> (AppState, RuntimeOwner, tempfile::TempDir) {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("lifecycle_test.db");
    let storage_dir = temp.path().join("storage");
    std::fs::create_dir_all(&storage_dir).unwrap();

    let mut config = AppConfig::default();
    config.database.url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    config.filesystem.default_local_root = storage_dir;

    let db = init_db(&config.database.url).await.unwrap();
    let built = build_application(config, db).await;

    (built.state, built.runtime, temp)
}

#[tokio::test]
async fn test_app_runtime_task_tracker_and_cancellation_drain() {
    let (_state, runtime, _temp) = setup_test_context().await;

    let task_executed = Arc::new(AtomicBool::new(false));
    let task_cleaned_up = Arc::new(AtomicBool::new(false));

    let executed_clone = task_executed.clone();
    let cleaned_clone = task_cleaned_up.clone();
    let token = runtime.shutdown_token.clone();

    runtime.task_tracker.spawn(async move {
        executed_clone.store(true, Ordering::SeqCst);
        tokio::select! {
            _ = token.cancelled() => {
                tokio::time::sleep(Duration::from_millis(50)).await;
                cleaned_clone.store(true, Ordering::SeqCst);
            }
            _ = tokio::time::sleep(Duration::from_secs(60)) => {}
        }
    });

    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(task_executed.load(Ordering::SeqCst), "Task should be running");
    assert!(
        !task_cleaned_up.load(Ordering::SeqCst),
        "Task should not be cleaned up yet"
    );

    runtime.shutdown_token.cancel();
    runtime.task_tracker.close();

    let drain_result =
        tokio::time::timeout(Duration::from_secs(2), runtime.task_tracker.wait()).await;
    assert!(
        drain_result.is_ok(),
        "Task tracker drain should complete within deadline"
    );
    assert!(
        task_cleaned_up.load(Ordering::SeqCst),
        "Task should have cleanly executed its shutdown branch"
    );
}

#[tokio::test]
async fn test_graceful_shutdown_idle_server_terminates_promptly() {
    let (state, runtime, _temp) = setup_test_context().await;
    let app = create_router(state);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local_addr = listener.local_addr().unwrap();

    let shutdown_token = runtime.shutdown_token.clone();
    let token_for_shutdown = shutdown_token.clone();

    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                token_for_shutdown.cancelled().await;
            })
            .await
    });

    let stream = tokio::net::TcpStream::connect(local_addr).await;
    assert!(
        stream.is_ok(),
        "Server should accept incoming TCP connections"
    );
    drop(stream);

    let start = std::time::Instant::now();
    shutdown_token.cancel();

    let server_result = tokio::time::timeout(Duration::from_secs(2), server_handle).await;
    assert!(
        server_result.is_ok(),
        "Server should terminate cleanly under 2 seconds"
    );
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_secs(1),
        "Idle server shutdown should finish in < 1s, took {:?}",
        elapsed
    );
}

#[tokio::test]
async fn test_daemon_lock_lifecycle_acquire_and_release() {
    let temp = tempdir().unwrap();
    let lock_file = temp.path().join("aerofs.lock");

    let lock1 = DaemonLock::acquire(&lock_file);
    assert!(lock1.is_ok(), "First lock acquisition should succeed");
    let lock1 = lock1.unwrap();

    let lock2 = DaemonLock::acquire(&lock_file);
    assert!(
        lock2.is_err(),
        "Second lock acquisition should fail while first is held"
    );

    lock1.release();

    let lock3 = DaemonLock::acquire(&lock_file);
    assert!(
        lock3.is_ok(),
        "Lock acquisition should succeed after release"
    );
    lock3.unwrap().release();
}

#[tokio::test]
async fn test_runtime_phase_transitions_and_health_readiness() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use backend::state::RuntimePhase;
    use tower::ServiceExt;

    let (state, runtime, _temp) = setup_test_context().await;
    let app = create_router(state);

    assert_eq!(runtime.phase(), RuntimePhase::Starting);
    let req = Request::builder()
        .uri("/health/ready")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);

    runtime.set_phase(RuntimePhase::Running);
    assert_eq!(runtime.phase(), RuntimePhase::Running);
    let req = Request::builder()
        .uri("/health/ready")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    runtime.set_phase(RuntimePhase::ShuttingDown);
    assert!(runtime.is_shutting_down());
    let req = Request::builder()
        .uri("/health/ready")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn test_shutdown_guard_rejects_mutations_with_503() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use backend::state::RuntimePhase;
    use tower::ServiceExt;

    let (state, runtime, _temp) = setup_test_context().await;
    let app = create_router(state);

    runtime.set_phase(RuntimePhase::Running);
    let req = Request::builder()
        .uri("/health/live")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    runtime.set_phase(RuntimePhase::ShuttingDown);

    let mutation_req = Request::builder()
        .uri("/api/v1/connections")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let resp = app.clone().oneshot(mutation_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(resp.headers().get("Retry-After").unwrap(), "5");

    let readonly_req = Request::builder()
        .uri("/health/live")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(readonly_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_transfer_submit_job_rejected_during_shutdown() {
    use backend::application::transfers::CreateTransferCommand;
    use backend::domain::{Actor, ConnectionId};
    use backend::ports::transfer::TransferType;
    use backend::state::RuntimePhase;

    let (state, runtime, _temp) = setup_test_context().await;
    runtime.set_phase(RuntimePhase::Running);
    let transfers = TransferState::from_ref(&state);

    runtime.shutdown_token.cancel();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let actor = Actor {
        id: "shutdown-test-admin".to_string(),
        username: "admin".to_string(),
        is_admin: true,
    };
    let res = transfers
        .use_cases
        .create_transfer
        .execute(
            &actor,
            CreateTransferCommand {
                name: "test_shutdown_job".to_string(),
                transfer_type: TransferType::Copy,
                source_connection: ConnectionId::local(),
                source_path: "/src.txt".to_string(),
                destination_connection: ConnectionId::local(),
                destination_path: "/dst.txt".to_string(),
            },
        )
        .await;

    assert!(
        res.is_err(),
        "transfer submission must be rejected when server is shutting down"
    );
    assert!(
        res.unwrap_err().to_string().contains("shutting down"),
        "Error message should explain server is shutting down"
    );
}

#[tokio::test]
async fn test_shutdown_guard_exact_path_classification() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use backend::state::RuntimePhase;
    use tower::ServiceExt;

    let (state, runtime, _temp) = setup_test_context().await;
    let app = create_router(state);

    runtime.set_phase(RuntimePhase::ShuttingDown);

    let fake_cancel_req = Request::builder()
        .uri("/api/v1/files/cancelled-dir")
        .method("POST")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(fake_cancel_req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "Substring /cancel in non-transfer path must be rejected during shutdown"
    );

    let real_cancel_req = Request::builder()
        .uri("/api/v1/transfers/job_abc123/cancel")
        .method("POST")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(real_cancel_req).await.unwrap();
    assert_ne!(
        resp.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "Transfer cancel endpoint must be allowed through during shutdown"
    );

    let logout_req = Request::builder()
        .uri("/api/v1/auth/logout")
        .method("POST")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(logout_req).await.unwrap();
    assert_ne!(
        resp.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "Auth logout endpoint must be allowed through during shutdown"
    );
}

#[tokio::test]
async fn test_shutdown_reason_and_single_owner_coordinator() {
    use backend::state::{RuntimePhase, ShutdownReason};

    let (_state, runtime, _temp) = setup_test_context().await;
    assert_eq!(runtime.phase(), RuntimePhase::Starting);
    assert_eq!(runtime.shutdown_reason(), None);

    runtime.request_shutdown(ShutdownReason::Sigterm);

    assert_eq!(runtime.phase(), RuntimePhase::ShuttingDown);
    assert_eq!(runtime.shutdown_reason(), Some(ShutdownReason::Sigterm));
    assert!(runtime.shutdown_token.is_cancelled());
}

#[tokio::test]
async fn test_first_shutdown_reason_wins_compare_exchange() {
    use backend::state::{RuntimePhase, ShutdownReason};

    let (_state, runtime, _temp) = setup_test_context().await;

    let first = runtime.request_shutdown(ShutdownReason::CtrlC);
    assert!(first, "First shutdown request must return true");
    assert_eq!(runtime.shutdown_reason(), Some(ShutdownReason::CtrlC));
    assert_eq!(runtime.phase(), RuntimePhase::ShuttingDown);

    let second = runtime.request_shutdown(ShutdownReason::Sigterm);
    assert!(!second, "Subsequent shutdown request must return false");
    assert_eq!(
        runtime.shutdown_reason(),
        Some(ShutdownReason::CtrlC),
        "Initial reason must be preserved"
    );

    let third = runtime.request_shutdown(ShutdownReason::Internal);
    assert!(!third);
    assert_eq!(runtime.shutdown_reason(), Some(ShutdownReason::CtrlC));
}

#[tokio::test]
async fn test_shutdown_guard_rejects_new_websocket_during_shutdown() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use backend::state::RuntimePhase;
    use tower::ServiceExt;

    let (state, runtime, _temp) = setup_test_context().await;
    let app = create_router(state);

    runtime.set_phase(RuntimePhase::ShuttingDown);

    let ws_req = Request::builder()
        .uri("/api/v1/ws")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(ws_req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "New WebSocket connection attempts during shutdown drain must be rejected with 503"
    );
    assert_eq!(resp.headers().get("Retry-After").unwrap(), "5");
}
