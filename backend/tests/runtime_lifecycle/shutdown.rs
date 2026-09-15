use axum::{
    body::Body,
    extract::FromRef,
    http::{header, Request, StatusCode},
};
use backend::{
    application::transfers::CreateTransferCommand,
    domain::{Actor, ConnectionId},
    ports::transfer::TransferType,
    state::{RuntimePhase, ShutdownReason, TransferState},
};
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tower::ServiceExt;

use crate::support::TestAppBuilder;

#[tokio::test]
async fn health_readiness_tracks_starting_running_and_shutdown_phases() {
    let app = TestAppBuilder::new().build().await;
    assert_eq!(app.runtime.phase(), RuntimePhase::Starting);

    let ready = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ready.status(), StatusCode::SERVICE_UNAVAILABLE);

    app.runtime.set_phase(RuntimePhase::Running);
    let ready = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ready.status(), StatusCode::OK);

    app.runtime.request_shutdown(ShutdownReason::Manual);
    let ready = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ready.status(), StatusCode::SERVICE_UNAVAILABLE);

    let live = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health/live")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(live.status(), StatusCode::OK);
}

#[tokio::test]
async fn shutdown_guard_uses_exact_exceptions_and_rejects_new_websockets() {
    let app = TestAppBuilder::new().running().build().await;
    app.runtime.set_phase(RuntimePhase::ShuttingDown);

    let fake_cancel = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/files/cancelled-dir")
                .method("POST")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fake_cancel.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(fake_cancel.headers().get(header::RETRY_AFTER).unwrap(), "5");

    let transfer_cancel = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/transfers/job_abc123/cancel")
                .method("POST")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(transfer_cancel.status(), StatusCode::SERVICE_UNAVAILABLE);

    let logout = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/logout")
                .method("POST")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(logout.status(), StatusCode::SERVICE_UNAVAILABLE);

    let websocket = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/ws")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(websocket.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(websocket.headers().get(header::RETRY_AFTER).unwrap(), "5");
}

#[tokio::test]
async fn application_transfer_submission_is_rejected_immediately_after_shutdown_request() {
    let app = TestAppBuilder::new()
        .running()
        .with_file("src.txt", b"shutdown payload".to_vec())
        .build()
        .await;
    let transfers = TransferState::from_ref(&app.state);
    let actor = Actor {
        id: "shutdown-admin".to_string(),
        username: "admin".to_string(),
        is_admin: true,
    };

    assert!(app.runtime.request_shutdown(ShutdownReason::Manual));
    let result = transfers
        .use_cases
        .create_transfer
        .execute(
            &actor,
            CreateTransferCommand {
                name: "rejected-during-shutdown".to_string(),
                transfer_type: TransferType::Copy,
                source_connection: ConnectionId::local(),
                source_path: "/src.txt".to_string(),
                destination_connection: ConnectionId::local(),
                destination_path: "/dst.txt".to_string(),
            },
        )
        .await;

    let error = result.expect_err("new transfer must be rejected after shutdown begins");
    assert!(error.to_string().contains("shutting down"));
}

#[tokio::test]
async fn idle_http_server_exits_within_shutdown_deadline() {
    let app = TestAppBuilder::new().running().build().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = app.router.clone();
    let shutdown = app.runtime.shutdown_token.clone();

    let server = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                shutdown.cancelled().await;
            })
            .await
    });

    let stream = TcpStream::connect(address)
        .await
        .expect("server should accept a TCP connection");
    drop(stream);

    assert!(app.runtime.request_shutdown(ShutdownReason::Manual));
    let result = tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .expect("idle server should stop before shutdown deadline")
        .expect("server task should join cleanly");
    result.expect("graceful server shutdown should succeed");
}
