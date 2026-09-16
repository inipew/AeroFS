use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

use crate::support::TestAppBuilder;

#[tokio::test]
async fn file_crud_preserves_conflict_envelope_and_listing_contract() {
    let app = TestAppBuilder::new().running().build().await;
    let cookie = app.admin_cookie().await;

    let mkdir = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/connections/local/directories")
                .method("POST")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"path": "/projects"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(mkdir.status(), StatusCode::CREATED);

    let create = || {
        Request::builder()
            .uri("/api/v1/connections/local/files")
            .method("POST")
            .header(header::COOKIE, &cookie)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({"path": "/projects/notes.txt"}).to_string(),
            ))
            .unwrap()
    };

    let created = app.router.clone().oneshot(create()).await.unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);

    let duplicate = app.router.clone().oneshot(create()).await.unwrap();
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);
    let body: Value = serde_json::from_slice(
        &to_bytes(duplicate.into_body(), usize::MAX).await.unwrap(),
    )
    .unwrap();
    assert_eq!(body["error"]["code"], "ALREADY_EXISTS");
    assert_eq!(body["error"]["category"], "conflict");

    let listing = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/connections/local/files?path=/projects")
                .method("GET")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(listing.status(), StatusCode::OK);
    let listing: Value = serde_json::from_slice(
        &to_bytes(listing.into_body(), usize::MAX).await.unwrap(),
    )
    .unwrap();
    assert_eq!(listing["total_count"], 1);
    assert_eq!(listing["entries"][0]["name"], "notes.txt");

    let deleted = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/connections/local/files")
                .method("DELETE")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"paths": ["/projects/notes.txt"]}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(deleted.status(), StatusCode::OK);
}

#[tokio::test]
async fn oversized_editor_write_returns_payload_too_large() {
    let app = TestAppBuilder::new().running().build().await;
    let cookie = app.admin_cookie().await;

    let created = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/connections/local/files")
                .method("POST")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"path": "/big.txt"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);

    let oversized = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/connections/local/files/content")
                .method("PUT")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "path": "/big.txt",
                        "content": "A".repeat(15 * 1024 * 1024)
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(oversized.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn editor_preview_content_is_isolated_by_security_headers() {
    let app = TestAppBuilder::new()
        .running()
        .with_file("vector.svg", b"<svg><script>alert(1)</script></svg>".to_vec())
        .build()
        .await;
    let cookie = app.admin_cookie().await;

    let response = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/connections/local/files/content?path=/vector.svg")
                .method("GET")
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("content-security-policy")
            .unwrap()
            .to_str()
            .unwrap(),
        "default-src 'none'; sandbox"
    );
    assert_eq!(
        response
            .headers()
            .get("x-content-type-options")
            .unwrap()
            .to_str()
            .unwrap(),
        "nosniff"
    );
}
