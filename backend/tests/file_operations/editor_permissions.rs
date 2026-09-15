use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

use crate::support::TestAppBuilder;

#[cfg(unix)]
#[tokio::test]
async fn editor_save_preserves_destination_permissions() {
    let app = TestAppBuilder::new()
        .running()
        .with_file("secure.conf", b"initial".to_vec())
        .build()
        .await;
    let cookie = app.login_admin().await;

    let chmod = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/connections/local/files/chmod")
                .method("POST")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"path": "/secure.conf", "mode": 0o600}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(chmod.status(), StatusCode::OK);

    let updated = app
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
                        "path": "/secure.conf",
                        "content": "SECRET_KEY=ABCDEF123456"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(updated.status(), StatusCode::OK);

    let metadata = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/connections/local/files/metadata?path=/secure.conf")
                .method("GET")
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(metadata.status(), StatusCode::OK);
    let metadata: Value = serde_json::from_slice(
        &to_bytes(metadata.into_body(), usize::MAX).await.unwrap(),
    )
    .unwrap();
    assert_eq!(metadata["permissions"], "0600");
}
