use axum::{
    body::{to_bytes, Body},
    http::{header, HeaderMap, HeaderValue, Method, Request, StatusCode},
};
use backend::state::RuntimePhase;
use serde_json::{json, Value};
use tower::ServiceExt;

use crate::support::{response_json, TestAppBuilder, TestAuth};

#[tokio::test]
async fn openapi_surface_exposes_supported_auth_and_major_routes() {
    let app = TestAppBuilder::new().running().build().await;
    let response = app
        .json_request(Method::GET, "/openapi.json", TestAuth::Anonymous, None)
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;

    assert_eq!(body["openapi"], "3.1.0");
    assert_eq!(body["info"]["title"], "AeroFS API");
    let paths = body["paths"].as_object().unwrap();
    for path in [
        "/api/v1/auth/login",
        "/api/v1/connections",
        "/api/v1/connections/{id}/files",
        "/api/v1/transfers",
        "/api/v1/sync",
        "/api/v1/shares",
        "/api/v1/trash",
        "/api/v1/user/preferences",
        "/api/v1/settings",
        "/api/v1/audit-logs",
    ] {
        assert!(paths.contains_key(path), "OpenAPI is missing {path}");
    }
    assert!(body["components"]["securitySchemes"]["CookieAuth"].is_object());
    assert!(body["components"]["securitySchemes"]["BearerAuth"].is_object());
}

#[tokio::test]
async fn malformed_json_returns_the_standard_validation_envelope() {
    let app = TestAppBuilder::new().running().build().await;
    let request = Request::builder()
        .uri("/api/v1/auth/login")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{ invalid_json: true "))
        .unwrap();
    let response = app.router.clone().oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response_json(response).await;
    assert_eq!(body["error"]["code"], "BAD_REQUEST");
    assert_eq!(body["error"]["category"], "validation");
    assert!(body["error"]["message"].is_string());
}

#[tokio::test]
async fn unknown_api_route_returns_the_standard_not_found_envelope() {
    let app = TestAppBuilder::new().running().build().await;
    let response = app
        .json_request(
            Method::GET,
            "/api/v1/does-not-exist",
            TestAuth::Anonymous,
            None,
        )
        .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = response_json(response).await;
    assert_eq!(body["error"]["code"], "NOT_FOUND");
    assert_eq!(body["error"]["category"], "not_found");
    assert!(body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("does-not-exist"));
}

#[tokio::test]
async fn method_not_allowed_preserves_allow_and_uses_standard_error_envelope() {
    let app = TestAppBuilder::new().running().build().await;
    let response = app
        .json_request(Method::PUT, "/health", TestAuth::Anonymous, None)
        .await;

    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert!(response.headers().contains_key(header::ALLOW));
    let body = response_json(response).await;
    assert_eq!(body["error"]["code"], "METHOD_NOT_ALLOWED");
    assert_eq!(body["error"]["category"], "validation");
}

#[tokio::test]
async fn shutdown_guard_rejects_mutations_but_keeps_logout_available() {
    let app = TestAppBuilder::new().running().build().await;
    let session = app.login_session("admin", "admin12345").await;
    app.runtime.set_phase(RuntimePhase::ShuttingDown);

    let rejected = app
        .json_request(
            Method::POST,
            "/api/v1/connections",
            TestAuth::Cookie(&session),
            Some(json!({
                "name": "Blocked During Shutdown",
                "provider": "local",
                "base_path": "/tmp"
            })),
        )
        .await;
    assert_eq!(rejected.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(rejected.headers().get(header::RETRY_AFTER).unwrap(), "5");
    let body = response_json(rejected).await;
    assert_eq!(body["error"]["code"], "SERVICE_UNAVAILABLE");

    let logout = app
        .json_request(
            Method::POST,
            "/api/v1/auth/logout",
            TestAuth::Cookie(&session),
            None,
        )
        .await;
    assert_eq!(logout.status(), StatusCode::OK);
}

#[tokio::test]
async fn security_headers_are_global_and_https_requests_receive_hsts() {
    let app = TestAppBuilder::new().running().build().await;
    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-proto", HeaderValue::from_static("https"));
    let response = app
        .json_request_with_headers(
            Method::GET,
            "/health/live",
            TestAuth::Anonymous,
            headers,
            None,
        )
        .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers().get("x-content-type-options").unwrap(), "nosniff");
    assert_eq!(response.headers().get("x-frame-options").unwrap(), "SAMEORIGIN");
    assert_eq!(
        response.headers().get("referrer-policy").unwrap(),
        "strict-origin-when-cross-origin"
    );
    assert_eq!(
        response.headers().get("strict-transport-security").unwrap(),
        "max-age=63072000; includeSubDomains; preload"
    );
}

#[tokio::test]
async fn idempotency_replays_the_original_mutation_response() {
    let app = TestAppBuilder::new().running().build().await;
    let session = app.login_session("admin", "admin12345").await;
    let mut headers = HeaderMap::new();
    headers.insert(
        "idempotency-key",
        HeaderValue::from_static("phase6-create-directory"),
    );

    let first = app
        .json_request_with_headers(
            Method::POST,
            "/api/v1/connections/local/directories",
            TestAuth::Cookie(&session),
            headers.clone(),
            Some(json!({"path": "/idempotent"})),
        )
        .await;
    assert_eq!(first.status(), StatusCode::CREATED);

    let second = app
        .json_request_with_headers(
            Method::POST,
            "/api/v1/connections/local/directories",
            TestAuth::Cookie(&session),
            headers,
            Some(json!({"path": "/idempotent"})),
        )
        .await;
    assert_eq!(second.status(), StatusCode::CREATED);
    assert_eq!(second.headers().get("x-cache-idempotency").unwrap(), "HIT");
}

#[tokio::test]
async fn health_contract_reports_live_and_ready_states() {
    let app = TestAppBuilder::new().running().build().await;
    let live = app
        .json_request(Method::GET, "/health/live", TestAuth::Anonymous, None)
        .await;
    assert_eq!(live.status(), StatusCode::OK);
    let live = response_json(live).await;
    assert_eq!(live["status"], "alive");

    let ready = app
        .json_request(Method::GET, "/health/ready", TestAuth::Anonymous, None)
        .await;
    assert_eq!(ready.status(), StatusCode::OK);
    let ready = response_json(ready).await;
    assert_eq!(ready["status"], "ready");
    assert_eq!(ready["database"], "connected");
}
