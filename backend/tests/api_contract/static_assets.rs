use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use tower::ServiceExt;

use crate::support::TestAppBuilder;

#[tokio::test]
async fn embedded_frontend_serves_root_and_spa_fallback_routes() {
    let app = TestAppBuilder::new().running().build().await;

    let root = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(root.status(), StatusCode::OK);
    assert!(root
        .headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap()
        .contains("text/html"));
    let body = to_bytes(root.into_body(), 1024 * 1024).await.unwrap();
    let html = String::from_utf8_lossy(&body);
    assert!(
        html.contains("<!DOCTYPE html>")
            || html.contains("<html")
            || html.contains("AeroFS")
            || html.contains("id=\"app\"")
    );

    let fallback = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/browse/some/deep/folder")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fallback.status(), StatusCode::OK);
    assert!(fallback
        .headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap()
        .contains("text/html"));
}
