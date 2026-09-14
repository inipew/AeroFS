use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

use crate::support::TestAppBuilder;

async fn json_body(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

#[tokio::test]
async fn admin_can_create_list_get_and_delete_remote_connection_without_exposing_secret() {
    let app = TestAppBuilder::new().running().build().await;
    let cookie = app.login_admin().await;

    let create = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/connections")
                .method("POST")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "name": "Object Storage",
                        "provider": "s3",
                        "username": "access-key",
                        "secret": "super-secret",
                        "base_path": "/backups"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::CREATED);
    let created = json_body(create).await;
    let id = created["id"].as_str().unwrap().to_string();
    assert!(created.get("secret").is_none());

    let list = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/connections")
                .method("GET")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let list = json_body(list).await;
    assert!(list
        .as_array()
        .unwrap()
        .iter()
        .any(|connection| connection["id"] == id));
    assert!(list
        .as_array()
        .unwrap()
        .iter()
        .all(|connection| connection.get("secret").is_none()));

    let detail = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/connections/{id}"))
                .method("GET")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(detail.status(), StatusCode::OK);
    let detail = json_body(detail).await;
    assert_eq!(detail["connection"]["provider"], "s3");
    assert_eq!(detail["connection"]["name"], "Object Storage");
    assert!(detail["connection"].get("secret").is_none());
    assert_eq!(detail["capabilities"]["read"], true);

    let delete = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/connections/{id}"))
                .method("DELETE")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete.status(), StatusCode::OK);

    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM connections WHERE id = ?")
        .bind(&id)
        .fetch_one(&app.db)
        .await
        .unwrap();
    let credential: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM connection_credentials WHERE connection_id = ?")
            .bind(&id)
            .fetch_one(&app.db)
            .await
            .unwrap();
    assert_eq!(row.0, 0);
    assert_eq!(credential.0, 0);
}

#[tokio::test]
async fn sftp_connection_kind_round_trips_through_http_detail_contract() {
    let app = TestAppBuilder::new().running().build().await;
    let cookie = app.login_admin().await;
    let create = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/connections")
                .method("POST")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "name": "SFTP Storage",
                        "provider": "sftp",
                        "host": "127.0.0.1",
                        "port": 22,
                        "username": "root",
                        "base_path": "/srv/data"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::CREATED);
    let created = json_body(create).await;
    let id = created["id"].as_str().unwrap();

    let detail = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/connections/{id}"))
                .method("GET")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(detail.status(), StatusCode::OK);
    let detail = json_body(detail).await;
    assert_eq!(detail["connection"]["provider"], "sftp");
    assert_eq!(detail["connection"]["host"], "127.0.0.1");
    assert_eq!(detail["connection"]["port"], 22);
    assert_eq!(detail["capabilities"]["read"], true);
}

#[tokio::test]
async fn local_connection_test_endpoint_is_immediate_and_successful() {
    let app = TestAppBuilder::new().running().build().await;
    let cookie = app.login_admin().await;
    let response = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/connections/local/test")
                .method("POST")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["latency_ms"], 0);
}

#[tokio::test]
async fn unauthenticated_connection_creation_is_rejected() {
    let app = TestAppBuilder::new().running().build().await;
    let response = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/connections")
                .method("POST")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"name": "Unauthorized", "provider": "s3"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
