use axum::{body::to_bytes, http::{Method, StatusCode}};
use serde_json::json;

use crate::support::{response_json, TestAppBuilder, TestAuth};

#[tokio::test]
async fn password_protected_share_requires_the_correct_public_credential() {
    let app = TestAppBuilder::new()
        .running()
        .with_file("shared_file.txt", b"Secret shared contents".to_vec())
        .build()
        .await;
    let admin = app.admin_session().await;

    let create = app
        .json_request(
            Method::POST,
            "/api/v1/shares",
            TestAuth::Cookie(&admin),
            Some(json!({
                "connection_id": "local",
                "path": "/shared_file.txt",
                "password": "mypassword123",
                "expires_in_hours": 24
            })),
        )
        .await;
    assert_eq!(create.status(), StatusCode::CREATED);
    let created = response_json(create).await;
    let token = created["share_token"].as_str().unwrap().to_string();
    assert_eq!(created["has_password"], true);

    let missing = app
        .json_request(
            Method::GET,
            &format!("/api/v1/shares/public/{token}"),
            TestAuth::Anonymous,
            None,
        )
        .await;
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

    let wrong = app
        .json_request(
            Method::GET,
            &format!("/api/v1/shares/public/{token}?password=wrongpassword"),
            TestAuth::Anonymous,
            None,
        )
        .await;
    assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);

    let correct = app
        .json_request(
            Method::GET,
            &format!("/api/v1/shares/public/{token}?password=mypassword123"),
            TestAuth::Anonymous,
            None,
        )
        .await;
    assert_eq!(correct.status(), StatusCode::OK);
    let bytes = to_bytes(correct.into_body(), usize::MAX).await.unwrap();
    assert_eq!(bytes.as_ref(), b"Secret shared contents");
}
