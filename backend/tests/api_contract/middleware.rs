use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use backend::middleware::REQUEST_ID_HEADER;
use serde_json::{json, Value};
use tower::ServiceExt;

use crate::support::TestAppBuilder;

#[tokio::test]
async fn request_id_is_generated_and_existing_trace_id_is_preserved() {
    let app = TestAppBuilder::new().running().build().await;

    let generated = app
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
    assert_eq!(generated.status(), StatusCode::OK);
    let generated_id = generated
        .headers()
        .get(REQUEST_ID_HEADER)
        .expect("response must include x-request-id")
        .to_str()
        .unwrap();
    assert!(!generated_id.is_empty());

    let custom_id = "custom-trace-id-abc123xyz";
    let propagated = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health/live")
                .header(REQUEST_ID_HEADER, custom_id)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(propagated.status(), StatusCode::OK);
    assert_eq!(
        propagated
            .headers()
            .get(REQUEST_ID_HEADER)
            .unwrap()
            .to_str()
            .unwrap(),
        custom_id
    );
}

#[tokio::test]
async fn transfer_submission_idempotency_replays_the_original_job() {
    let app = TestAppBuilder::new()
        .running()
        .with_file("source.txt", b"idempotent transfer payload".to_vec())
        .build()
        .await;
    let cookie = app.login_admin().await;
    let body = json!({
        "name": "idempotent transfer",
        "transfer_type": "copy",
        "source_connection_id": "local",
        "source_path": "/source.txt",
        "destination_connection_id": "local",
        "destination_path": "/destination.txt"
    });

    let submit = |router: axum::Router| {
        let cookie = cookie.clone();
        let body = body.clone();
        async move {
            router
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/v1/transfers")
                        .header(header::COOKIE, cookie)
                        .header(header::CONTENT_TYPE, "application/json")
                        .header("Idempotency-Key", "transfer-submit-123")
                        .body(Body::from(serde_json::to_vec(&body).unwrap()))
                        .unwrap(),
                )
                .await
                .unwrap()
        }
    };

    let first = submit(app.router.clone()).await;
    assert_eq!(first.status(), StatusCode::ACCEPTED);
    let first_body: Value = serde_json::from_slice(
        &to_bytes(first.into_body(), usize::MAX).await.unwrap(),
    )
    .unwrap();
    let first_id = first_body["job_id"].as_str().unwrap().to_string();

    let second = submit(app.router.clone()).await;
    assert_eq!(second.status(), StatusCode::ACCEPTED);
    assert_eq!(second.headers().get("x-cache-idempotency").unwrap(), "HIT");
    let second_body: Value = serde_json::from_slice(
        &to_bytes(second.into_body(), usize::MAX).await.unwrap(),
    )
    .unwrap();
    assert_eq!(second_body["job_id"].as_str(), Some(first_id.as_str()));
}
