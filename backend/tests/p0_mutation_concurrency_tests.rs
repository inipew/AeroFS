use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use backend::{
    bootstrap::build_application, config::AppConfig, create_router, db::init_db,
    state::RuntimePhase,
};
use serde_json::json;
use tempfile::tempdir;
use tower::ServiceExt;

async fn setup_test_app() -> axum::Router {
    let temp = tempdir().unwrap();
    let root = temp.keep();
    let db_path = root.join("p0-concurrency.db");
    let storage_dir = root.join("storage");
    std::fs::create_dir_all(&storage_dir).unwrap();

    let mut config = AppConfig::default();
    config.database.url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    config.filesystem.default_local_root = storage_dir;

    let db = init_db(&config.database.url).await.unwrap();
    let built = build_application(config, db).await;
    built.runtime.set_phase(RuntimePhase::Running);
    create_router(built.state)
}

async fn login_admin(app: &axum::Router) -> String {
    let request = Request::builder()
        .uri("/api/v1/auth/login")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "username": "admin", "password": "admin12345" }).to_string(),
        ))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    response
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

fn editor_put(cookie: &str, etag: &str, content: &str) -> Request<Body> {
    Request::builder()
        .uri("/api/v1/connections/local/files/content")
        .method("PUT")
        .header(header::COOKIE, cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::IF_MATCH, etag)
        .body(Body::from(
            json!({ "path": "/race.txt", "content": content }).to_string(),
        ))
        .unwrap()
}

#[tokio::test]
async fn concurrent_writers_cannot_both_commit_the_same_etag() {
    let app = setup_test_app().await;
    let cookie = login_admin(&app).await;

    let create = Request::builder()
        .uri("/api/v1/connections/local/files")
        .method("POST")
        .header(header::COOKIE, &cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "path": "/race.txt" }).to_string()))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(create).await.unwrap().status(),
        StatusCode::CREATED
    );

    let seed = Request::builder()
        .uri("/api/v1/connections/local/files/content")
        .method("PUT")
        .header(header::COOKIE, &cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "path": "/race.txt", "content": "version-a" }).to_string(),
        ))
        .unwrap();
    let seed_response = app.clone().oneshot(seed).await.unwrap();
    assert_eq!(seed_response.status(), StatusCode::OK);
    let etag = seed_response
        .headers()
        .get(header::ETAG)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    let left = app.clone().oneshot(editor_put(&cookie, &etag, "writer-left"));
    let right = app.clone().oneshot(editor_put(&cookie, &etag, "writer-right"));
    let (left, right) = tokio::join!(left, right);
    let left = left.unwrap();
    let right = right.unwrap();
    let statuses = [left.status(), right.status()];

    assert_eq!(
        statuses.iter().filter(|status| **status == StatusCode::OK).count(),
        1,
        "exactly one writer may commit one ETag generation: {statuses:?}"
    );
    assert!(
        statuses.iter().any(|status| {
            *status == StatusCode::CONFLICT || *status == StatusCode::PRECONDITION_FAILED
        }),
        "losing writer must observe contention or a stale precondition: {statuses:?}"
    );

    let get = Request::builder()
        .uri("/api/v1/connections/local/files/content?path=/race.txt")
        .method("GET")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(get).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert!(body.as_ref() == b"writer-left" || body.as_ref() == b"writer-right");
}
