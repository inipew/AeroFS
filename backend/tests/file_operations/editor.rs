use axum::{
    body::{to_bytes, Body},
    http::{header, Request, Response, StatusCode},
};
use serde_json::json;
use tower::ServiceExt;

use crate::support::{TestApp, TestAppBuilder};

async fn put_content(
    app: &TestApp,
    cookie: &str,
    path: &str,
    content: &str,
    if_match: Option<&str>,
    force: bool,
) -> Response<Body> {
    let mut builder = Request::builder()
        .uri("/api/v1/connections/local/files/content")
        .method("PUT")
        .header(header::COOKIE, cookie)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(value) = if_match {
        builder = builder.header(header::IF_MATCH, value);
    }
    if force {
        builder = builder.header("X-Force-Overwrite", "true");
    }
    app.router
        .clone()
        .oneshot(
            builder
                .body(Body::from(
                    json!({"path": path, "content": content}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn get_content(app: &TestApp, cookie: &str, path: &str) -> Response<Body> {
    app.router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/connections/local/files/content?path={path}"
                ))
                .method("GET")
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

fn etag(response: &Response<Body>) -> String {
    response
        .headers()
        .get(header::ETAG)
        .expect("file response must include ETag")
        .to_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn editor_reads_are_non_cacheable_and_return_etag() {
    let app = TestAppBuilder::new()
        .running()
        .with_file("hello.txt", b"hello".to_vec())
        .build()
        .await;
    let cookie = app.login_admin().await;

    let response = get_content(&app, &cookie, "/hello.txt").await;
    assert_eq!(response.status(), StatusCode::OK);
    let cache_control = response
        .headers()
        .get(header::CACHE_CONTROL)
        .expect("Cache-Control header")
        .to_str()
        .unwrap();
    assert!(cache_control.contains("no-store"));
    assert!(!cache_control.contains("max-age=3600"));
    assert!(response.headers().contains_key(header::ETAG));
}

#[tokio::test]
async fn edit_save_reopen_returns_new_content_and_generation_immediately() {
    let app = TestAppBuilder::new()
        .running()
        .with_file("code.rs", b"version-a".to_vec())
        .build()
        .await;
    let cookie = app.login_admin().await;

    let initial = get_content(&app, &cookie, "/code.rs").await;
    let etag_a = etag(&initial);
    let body = to_bytes(initial.into_body(), usize::MAX).await.unwrap();
    assert_eq!(body.as_ref(), b"version-a");

    let updated = put_content(
        &app,
        &cookie,
        "/code.rs",
        "version-b-with-more-bytes",
        Some(&etag_a),
        false,
    )
    .await;
    assert_eq!(updated.status(), StatusCode::OK);
    let etag_b = etag(&updated);
    assert_ne!(etag_a, etag_b);

    let reopened = get_content(&app, &cookie, "/code.rs").await;
    assert_eq!(etag(&reopened), etag_b);
    let body = to_bytes(reopened.into_body(), usize::MAX).await.unwrap();
    assert_eq!(body.as_ref(), b"version-b-with-more-bytes");
}

#[tokio::test]
async fn stale_etag_is_rejected_after_external_mutation_without_scheduler_sleep() {
    let app = TestAppBuilder::new()
        .running()
        .with_file("conflict.txt", b"short".to_vec())
        .build()
        .await;
    let cookie = app.login_admin().await;
    let original = get_content(&app, &cookie, "/conflict.txt").await;
    let stale = etag(&original);

    std::fs::write(
        app.storage_path("conflict.txt"),
        b"external-change-with-a-different-length",
    )
    .unwrap();

    let response = put_content(
        &app,
        &cookie,
        "/conflict.txt",
        "stale editor update",
        Some(&stale),
        false,
    )
    .await;
    assert_eq!(response.status(), StatusCode::PRECONDITION_FAILED);
}

#[tokio::test]
async fn explicit_force_and_wildcard_preconditions_allow_overwrite() {
    let app = TestAppBuilder::new()
        .running()
        .with_file("force.txt", b"original".to_vec())
        .with_file("wildcard.txt", b"original".to_vec())
        .build()
        .await;
    let cookie = app.login_admin().await;

    let original = get_content(&app, &cookie, "/force.txt").await;
    let stale = etag(&original);
    std::fs::write(
        app.storage_path("force.txt"),
        b"external-change-with-different-length",
    )
    .unwrap();
    let forced = put_content(
        &app,
        &cookie,
        "/force.txt",
        "forced",
        Some(&stale),
        true,
    )
    .await;
    assert_eq!(forced.status(), StatusCode::OK);
    assert_eq!(std::fs::read(app.storage_path("force.txt")).unwrap(), b"forced");

    std::fs::write(app.storage_path("wildcard.txt"), b"external-change").unwrap();
    let wildcard = put_content(
        &app,
        &cookie,
        "/wildcard.txt",
        "wildcard overwrite",
        Some("*"),
        false,
    )
    .await;
    assert_eq!(wildcard.status(), StatusCode::OK);
    assert_eq!(
        std::fs::read(app.storage_path("wildcard.txt")).unwrap(),
        b"wildcard overwrite"
    );
}

#[tokio::test]
async fn concurrent_writers_cannot_commit_the_same_etag_generation_twice() {
    let app = TestAppBuilder::new()
        .running()
        .with_file("race.txt", b"seed".to_vec())
        .build()
        .await;
    let cookie = app.login_admin().await;
    let initial = get_content(&app, &cookie, "/race.txt").await;
    let generation = etag(&initial);

    let left = put_content(
        &app,
        &cookie,
        "/race.txt",
        "writer-left-content",
        Some(&generation),
        false,
    );
    let right = put_content(
        &app,
        &cookie,
        "/race.txt",
        "writer-right-content-different-size",
        Some(&generation),
        false,
    );
    let (left, right) = tokio::join!(left, right);
    let statuses = [left.status(), right.status()];

    assert_eq!(
        statuses
            .iter()
            .filter(|status| **status == StatusCode::OK)
            .count(),
        1,
        "exactly one writer may commit a generation: {statuses:?}"
    );
    assert!(statuses.iter().any(|status| {
        *status == StatusCode::CONFLICT || *status == StatusCode::PRECONDITION_FAILED
    }));

    let final_response = get_content(&app, &cookie, "/race.txt").await;
    let body = to_bytes(final_response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert!(
        body.as_ref() == b"writer-left-content"
            || body.as_ref() == b"writer-right-content-different-size"
    );
}

#[tokio::test]
async fn cors_preflight_allows_editor_concurrency_headers() {
    let app = TestAppBuilder::new().running().build().await;
    let response = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/connections/local/files/content")
                .method("OPTIONS")
                .header("Origin", "http://localhost:5173")
                .header("Access-Control-Request-Method", "PUT")
                .header(
                    "Access-Control-Request-Headers",
                    "if-match, x-force-overwrite, content-type",
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let allow = response
        .headers()
        .get("access-control-allow-headers")
        .expect("Access-Control-Allow-Headers")
        .to_str()
        .unwrap()
        .to_lowercase();
    assert!(allow.contains("if-match") || allow.contains('*'));
    assert!(allow.contains("x-force-overwrite") || allow.contains('*'));
}
