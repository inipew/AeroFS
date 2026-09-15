use axum::http::{header, Method, StatusCode};
use serde_json::json;

use crate::support::{response_json, TestAppBuilder, TestAuth};

#[tokio::test]
async fn invalid_credentials_return_structured_authentication_error() {
    let app = TestAppBuilder::new().running().build().await;
    let response = app
        .json_request(
            Method::POST,
            "/api/v1/auth/login",
            TestAuth::Anonymous,
            Some(json!({"username": "admin", "password": "definitely-wrong"})),
        )
        .await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = response_json(response).await;
    assert_eq!(body["error"]["code"], "INVALID_CREDENTIALS");
    assert_eq!(body["error"]["category"], "authentication");
    assert_eq!(body["error"]["retryable"], false);
    assert_eq!(
        body["error"]["user_action"],
        "check_username_and_password"
    );
}

#[tokio::test]
async fn login_cookie_and_bearer_identify_the_same_principal() {
    let app = TestAppBuilder::new().running().build().await;
    let session = app.login_session("admin", "admin12345").await;

    let cookie_attributes = session.set_cookie.to_ascii_lowercase();
    assert!(cookie_attributes.contains("session_id="));
    assert!(cookie_attributes.contains("httponly"));
    assert!(cookie_attributes.contains("path=/"));
    assert!(cookie_attributes.contains("samesite=lax"));
    assert!(cookie_attributes.contains("max-age="));

    let cookie_response = app
        .json_request(
            Method::GET,
            "/api/v1/auth/me",
            TestAuth::Cookie(&session),
            None,
        )
        .await;
    assert_eq!(cookie_response.status(), StatusCode::OK);
    let cookie_user = response_json(cookie_response).await;

    let bearer_response = app
        .json_request(
            Method::GET,
            "/api/v1/auth/me",
            TestAuth::Bearer(&session),
            None,
        )
        .await;
    assert_eq!(bearer_response.status(), StatusCode::OK);
    let bearer_user = response_json(bearer_response).await;

    assert_eq!(cookie_user, bearer_user);
    assert_eq!(cookie_user["username"], "admin");
    assert_eq!(cookie_user["is_admin"], true);
}

#[tokio::test]
async fn anonymous_profile_access_is_a_structured_session_error() {
    let app = TestAppBuilder::new().running().build().await;
    let response = app
        .json_request(
            Method::GET,
            "/api/v1/auth/me",
            TestAuth::Anonymous,
            None,
        )
        .await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = response_json(response).await;
    assert_eq!(body["error"]["code"], "SESSION_EXPIRED");
    assert_eq!(body["error"]["category"], "authentication");
    assert_eq!(body["error"]["user_action"], "re_login");
}

#[tokio::test]
async fn logout_invalidates_the_session_and_clears_the_cookie() {
    let app = TestAppBuilder::new().running().build().await;
    let session = app.login_session("admin", "admin12345").await;

    let logout = app
        .json_request(
            Method::POST,
            "/api/v1/auth/logout",
            TestAuth::Cookie(&session),
            None,
        )
        .await;
    assert_eq!(logout.status(), StatusCode::OK);
    let clear_cookie = logout
        .headers()
        .get(header::SET_COOKIE)
        .expect("logout must clear session cookie")
        .to_str()
        .unwrap()
        .to_ascii_lowercase();
    assert!(clear_cookie.contains("session_id="));
    assert!(clear_cookie.contains("max-age=0"));
    let body = response_json(logout).await;
    assert_eq!(body["success"], true);

    let after_logout = app
        .json_request(
            Method::GET,
            "/api/v1/auth/me",
            TestAuth::Cookie(&session),
            None,
        )
        .await;
    assert_eq!(after_logout.status(), StatusCode::UNAUTHORIZED);
    let body = response_json(after_logout).await;
    assert_eq!(body["error"]["code"], "SESSION_EXPIRED");
}

#[tokio::test]
async fn websocket_upgrade_requires_authentication() {
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    let app = TestAppBuilder::new().running().build().await;
    let request = Request::builder()
        .uri("/api/v1/ws")
        .method("GET")
        .header(header::UPGRADE, "websocket")
        .header(header::CONNECTION, "Upgrade")
        .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
        .header("Sec-WebSocket-Version", "13")
        .body(Body::empty())
        .unwrap();
    let response = app.router.clone().oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = response_json(response).await;
    assert_eq!(body["error"]["code"], "SESSION_EXPIRED");
}
