mod support;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use support::{eventually, TestAppBuilder, TestDatabase};
use tower::ServiceExt;

#[tokio::test]
async fn test_database_runs_real_migrations_without_seed_data() {
    let database = TestDatabase::migrated("migrated.db").await;

    let users: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&database.pool)
        .await
        .expect("users table must exist after migrations");
    assert_eq!(users.0, 0, "migrated fixture must not silently seed users");

    let migrations: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(&database.pool)
        .await
        .expect("sqlx migration history must exist");
    assert!(migrations.0 > 0, "real project migrations must have run");
    assert!(database.path.exists());
}

#[tokio::test]
async fn seeded_databases_clone_defaults_without_sharing_mutations() {
    let first = TestDatabase::seeded("seeded-first.db").await;
    let second = TestDatabase::seeded("seeded-second.db").await;

    assert_ne!(first.path, second.path);

    let first_defaults: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM users WHERE username = 'admin'), (SELECT COUNT(*) FROM connections WHERE id = 'local')",
    )
    .fetch_one(&first.pool)
    .await
    .expect("query first seeded defaults");
    let second_defaults: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM users WHERE username = 'admin'), (SELECT COUNT(*) FROM connections WHERE id = 'local')",
    )
    .fetch_one(&second.pool)
    .await
    .expect("query second seeded defaults");
    assert_eq!(first_defaults, (1, 1));
    assert_eq!(second_defaults, (1, 1));

    sqlx::query("UPDATE connections SET name = 'first-only' WHERE id = 'local'")
        .execute(&first.pool)
        .await
        .expect("mutate first seeded fixture");

    let second_local_name: (String,) =
        sqlx::query_as("SELECT name FROM connections WHERE id = 'local'")
            .fetch_one(&second.pool)
            .await
            .expect("query second local connection");
    assert_eq!(second_local_name.0, "Local Storage");
}

#[tokio::test]
async fn test_app_builder_owns_isolated_runtime_storage_and_authenticated_router() {
    let app = TestAppBuilder::new()
        .running()
        .with_file("/fixtures/hello.txt", b"hello from TestKit".to_vec())
        .build()
        .await;

    assert_eq!(
        std::fs::read(app.storage_path("/fixtures/hello.txt")).unwrap(),
        b"hello from TestKit"
    );
    assert!(app.database_path.exists());

    let cookie = app.login_admin().await;
    let request = Request::builder()
        .uri("/api/v1/auth/me")
        .method("GET")
        .header(header::COOKIE, cookie)
        .body(Body::empty())
        .unwrap();
    let response = app.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn eventually_observes_async_convergence_instead_of_assuming_a_delay() {
    let ready = Arc::new(AtomicBool::new(false));
    let producer_ready = Arc::clone(&ready);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(30)).await;
        producer_ready.store(true, Ordering::Release);
    });

    let observed = eventually(
        "producer readiness",
        Duration::from_secs(1),
        Duration::from_millis(5),
        || {
            let ready = Arc::clone(&ready);
            async move { ready.load(Ordering::Acquire).then_some(true) }
        },
    )
    .await;

    assert!(observed);
}
