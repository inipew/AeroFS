use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use backend::{config::AppConfig, create_router, db::init_db, state::RuntimePhase, AppState};
use serde_json::{json, Value};
use tempfile::tempdir;
use tower::ServiceExt;

async fn setup_test_app() -> (axum::Router, AppState, tempfile::TempDir) {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("test.db");
    let storage_dir = temp.path().join("storage");
    std::fs::create_dir_all(&storage_dir).unwrap();

    let mut config = AppConfig::default();
    config.database.url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    config.filesystem.default_local_root = storage_dir;

    let db = init_db(&config.database.url).await.unwrap();
    let state = AppState::new_with_db(config, db).await;
    state.runtime.set_phase(RuntimePhase::Running);
    let app = create_router(state.clone());

    (app, state, temp)
}

#[tokio::test]
async fn test_openapi_schema_coverage() {
    let (app, _state, _temp) = setup_test_app().await;

    let req = Request::builder()
        .uri("/openapi.json")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = to_bytes(resp.into_body(), 5 * 1024 * 1024).await.unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap();

    // Verify OpenAPI version and info
    assert_eq!(json["openapi"], "3.1.0");
    assert_eq!(json["info"]["title"], "AeroFS API");

    // Verify paths exist
    let paths = json["paths"].as_object().unwrap();
    assert!(paths.contains_key("/api/v1/auth/login"));
    assert!(paths.contains_key("/api/v1/connections"));
    assert!(paths.contains_key("/api/v1/connections/{id}/files"));
    assert!(paths.contains_key("/api/v1/transfers"));
    assert!(paths.contains_key("/api/v1/sync"));
    assert!(paths.contains_key("/api/v1/shares"));
    assert!(paths.contains_key("/api/v1/trash"));
    assert!(paths.contains_key("/api/v1/user/preferences"));
    assert!(paths.contains_key("/api/v1/settings"));
    assert!(paths.contains_key("/api/v1/audit-logs"));

    // Verify security schemes
    let security_schemes = &json["components"]["securitySchemes"];
    assert!(security_schemes["CookieAuth"].is_object());
    assert!(security_schemes["BearerAuth"].is_object());
}

#[tokio::test]
async fn test_malformed_json_returns_bad_request_error_response() {
    let (app, _state, _temp) = setup_test_app().await;

    // Send malformed JSON to /api/v1/auth/login
    let req = Request::builder()
        .uri("/api/v1/auth/login")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{ invalid_json: true "))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(json["error"]["code"], "BAD_REQUEST");
    assert!(json["error"]["message"].is_string());
}

#[tokio::test]
async fn test_api_not_found_returns_error_response_json() {
    let (app, _state, _temp) = setup_test_app().await;

    let req = Request::builder()
        .uri("/api/v1/some-route-that-does-not-exist")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(json["error"]["code"], "NOT_FOUND");
    assert!(json["error"]["message"]
        .as_str()
        .unwrap()
        .contains("some-route-that-does-not-exist"));
}

#[tokio::test]
async fn test_api_method_not_allowed_returns_error_response_and_allow_header() {
    let (app, _state, _temp) = setup_test_app().await;

    // PUT to /health (health only supports GET)
    let req = Request::builder()
        .uri("/health")
        .method("PUT")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);

    assert!(resp.headers().contains_key(header::ALLOW));

    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(json["error"]["code"], "METHOD_NOT_ALLOWED");
}

#[tokio::test]
async fn test_shutdown_guard_503_and_retry_after() {
    let (app, state, _temp) = setup_test_app().await;

    // Transition runtime phase to ShuttingDown
    state.runtime.set_phase(RuntimePhase::ShuttingDown);

    // Mutation request during shutdown
    let req = Request::builder()
        .uri("/api/v1/connections")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "name": "Local Storage",
                "provider": "local",
                "base_path": "/tmp"
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);

    let retry_after = resp.headers().get(header::RETRY_AFTER);
    assert!(retry_after.is_some());
    assert_eq!(retry_after.unwrap(), "5");

    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["error"]["code"], "SERVICE_UNAVAILABLE");
    assert!(json["error"]["message"]
        .as_str()
        .unwrap()
        .contains("shutting down"));
}
