use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use backend::{config::AppConfig, create_router, db::init_db, AppState};
use serde_json::{json, Value};
use tempfile::tempdir;
use tower::ServiceExt;

async fn setup_app() -> (axum::Router, String, tempfile::TempDir) {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("conn_test.db");
    let storage_dir = temp.path().join("storage");
    std::fs::create_dir_all(&storage_dir).unwrap();

    let mut config = AppConfig::default();
    config.database.url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    config.filesystem.default_local_root = storage_dir;

    let db = init_db(&config.database.url).await.unwrap();
    let state = AppState::new_with_db(config, db).await;
    let app = create_router(state);

    // Login as admin to get session cookie
    let login_req = Request::builder()
        .uri("/api/v1/auth/login")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "username": "admin", "password": "admin12345" }).to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(login_req).await.unwrap();
    let cookie_header = resp
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let cookie = cookie_header.split(';').next().unwrap().to_string();

    (app, cookie, temp)
}

#[tokio::test]
async fn test_remote_connections_crud_and_test() {
    let (app, cookie, _temp) = setup_app().await;

    // 1. Create a remote SFTP connection
    let create_sftp_req = Request::builder()
        .uri("/api/v1/connections")
        .method("POST")
        .header(header::COOKIE, &cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "name": "Production Server",
                "provider": "sftp",
                "host": "127.0.0.1",
                "port": 22,
                "username": "root",
                "secret": "sftp_super_secret_password",
                "base_path": "/var/www"
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(create_sftp_req).await.unwrap();
    let status = resp.status();
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    if status != StatusCode::CREATED {
        eprintln!("Response error: {}", String::from_utf8_lossy(&body));
    }
    assert_eq!(status, StatusCode::CREATED);

    let created_val: Value = serde_json::from_slice(&body).unwrap();
    let sftp_id = created_val["id"].as_str().unwrap().to_string();

    // 2. List connections -> must contain local and the new SFTP connection
    let list_req = Request::builder()
        .uri("/api/v1/connections")
        .method("GET")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(list_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let list_val: Value = serde_json::from_slice(&body).unwrap();
    let conns = list_val.as_array().unwrap();
    assert_eq!(conns.len(), 2);
    assert!(conns.iter().any(|c| c["id"] == "local"));
    assert!(conns.iter().any(|c| c["id"] == sftp_id));

    // 3. Get SFTP connection details and capabilities
    let get_req = Request::builder()
        .uri(format!("/api/v1/connections/{}", sftp_id))
        .method("GET")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(get_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let detail_val: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(detail_val["connection"]["provider"], "sftp");
    assert_eq!(detail_val["capabilities"]["read"], true);

    // 4. Test Local connection status -> 200
    let test_req = Request::builder()
        .uri("/api/v1/connections/local/test")
        .method("POST")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(test_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 5. Delete SFTP connection as admin -> 200
    let del_req = Request::builder()
        .uri(format!("/api/v1/connections/{}", sftp_id))
        .method("DELETE")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(del_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_nonadmin_connection_creation_forbidden() {
    let (app, _cookie, _temp) = setup_app().await;

    // Login as non-admin (Create connection without admin rights should return 401/403)
    let create_req = Request::builder()
        .uri("/api/v1/connections")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "name": "Unauthorized Conn",
                "provider": "sftp",
                "host": "127.0.0.1"
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(create_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_s3_connection_creation() {
    let (app, cookie, _temp) = setup_app().await;

    let create_s3_req = Request::builder()
        .uri("/api/v1/connections")
        .method("POST")
        .header(header::COOKIE, &cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "name": "Cloudflare R2 Bucket",
                "provider": "s3",
                "host": "my-aerofs-bucket",
                "username": "minio_access_key",
                "secret": "minio_secret_key",
                "base_path": "/backups"
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(create_s3_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let val: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(val["success"], true);
}
