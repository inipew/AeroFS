use axum::{
    body::{to_bytes, Body},
    extract::FromRef,
    http::{header, Request, StatusCode},
};
use backend::{
    config::AppConfig,
    create_router,
    db::{init_db, DbPool},
    domain::Actor,
    services::settings_service::UpdateSettingsRequest,
    state::SettingsState,
    transfer::{TransferJob, TransferStatus},
    AppState,
};
use serde_json::{json, Value};
use tempfile::tempdir;
use tower::ServiceExt;

struct TestApp {
    app: axum::Router,
    cookie: String,
    temp: tempfile::TempDir,
    state: AppState,
    db: DbPool,
}

async fn setup_app() -> TestApp {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("transfer_test.db");
    let storage_dir = temp.path().join("storage");
    std::fs::create_dir_all(&storage_dir).unwrap();
    std::fs::write(
        storage_dir.join("source.txt"),
        b"Hello World from Background Transfer Engine!",
    )
    .unwrap();

    let mut config = AppConfig::default();
    config.database.url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    config.filesystem.default_local_root = storage_dir;

    let db = init_db(&config.database.url).await.unwrap();
    let state = AppState::new_with_db(config, db.clone()).await;
    let app = create_router(state.clone());

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

    TestApp {
        app,
        cookie: cookie_header.split(';').next().unwrap().to_string(),
        temp,
        state,
        db,
    }
}

async fn submit_transfer(
    app: &axum::Router,
    cookie: &str,
    name: &str,
    transfer_type: &str,
    source: &str,
    destination: &str,
) -> String {
    let req = Request::builder()
        .uri("/api/v1/transfers")
        .method("POST")
        .header(header::COOKIE, cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "name": name,
                "transfer_type": transfer_type,
                "source_connection_id": "local",
                "source_path": source,
                "destination_connection_id": "local",
                "destination_path": destination
            })
            .to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let created: Value = serde_json::from_slice(&body).unwrap();
    created["job_id"].as_str().unwrap().to_string()
}

async fn list_transfers(app: &axum::Router, cookie: &str) -> Vec<TransferJob> {
    let req = Request::builder()
        .uri("/api/v1/transfers")
        .method("GET")
        .header(header::COOKIE, cookie)
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn wait_completed(app: &axum::Router, cookie: &str, job_id: &str) -> TransferJob {
    for _ in 0..50 {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        if let Some(job) = list_transfers(app, cookie)
            .await
            .into_iter()
            .find(|job| job.id == job_id)
        {
            if job.status == TransferStatus::Completed {
                return job;
            }
        }
    }
    panic!("transfer {job_id} did not complete");
}

#[tokio::test]
async fn test_transfer_engine_queue_and_execution() {
    let ctx = setup_app().await;
    let job_id = submit_transfer(
        &ctx.app,
        &ctx.cookie,
        "Copy source.txt to destination.txt",
        "copy",
        "/source.txt",
        "/destination.txt",
    )
    .await;
    let job = wait_completed(&ctx.app, &ctx.cookie, &job_id).await;
    assert!(job.transferred_bytes > 0);
}

#[tokio::test]
async fn test_transfer_cancellation_state_machine() {
    let ctx = setup_app().await;
    let job_id = submit_transfer(
        &ctx.app,
        &ctx.cookie,
        "Cancel Me Job",
        "copy",
        "/source.txt",
        "/cancel_dest.txt",
    )
    .await;

    let cancel_req = Request::builder()
        .uri(format!("/api/v1/transfers/{job_id}/cancel"))
        .method("POST")
        .header(header::COOKIE, &ctx.cookie)
        .body(Body::empty())
        .unwrap();
    let cancel_resp = ctx.app.clone().oneshot(cancel_req).await.unwrap();
    assert!(matches!(
        cancel_resp.status(),
        StatusCode::OK | StatusCode::CONFLICT
    ));

    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    let jobs = list_transfers(&ctx.app, &ctx.cookie).await;
    let job = jobs.iter().find(|job| job.id == job_id).unwrap();
    assert!(matches!(
        job.status,
        TransferStatus::Cancelled | TransferStatus::Completed
    ));
    if cancel_resp.status() == StatusCode::OK {
        assert_eq!(job.status, TransferStatus::Cancelled);
    }
}

#[tokio::test]
async fn test_transfer_safe_move_semantics() {
    let ctx = setup_app().await;
    let storage_dir = ctx.temp.path().join("storage");
    let move_src = storage_dir.join("to_move.txt");
    let move_dst = storage_dir.join("moved.txt");
    std::fs::write(&move_src, b"Move Me Transactionally!").unwrap();

    let job_id = submit_transfer(
        &ctx.app,
        &ctx.cookie,
        "Move to_move.txt to moved.txt",
        "move",
        "/to_move.txt",
        "/moved.txt",
    )
    .await;
    wait_completed(&ctx.app, &ctx.cookie, &job_id).await;

    assert!(!move_src.exists());
    assert!(move_dst.exists());
    assert_eq!(std::fs::read(&move_dst).unwrap(), b"Move Me Transactionally!");
}

#[tokio::test]
async fn test_transfer_sqlite_durability() {
    let ctx = setup_app().await;
    let job_id = submit_transfer(
        &ctx.app,
        &ctx.cookie,
        "Durable Job",
        "copy",
        "/source.txt",
        "/durable_dest.txt",
    )
    .await;
    wait_completed(&ctx.app, &ctx.cookie, &job_id).await;

    let row: (String, String, String) =
        sqlx::query_as("SELECT id, status, name FROM transfer_jobs WHERE id = ?")
            .bind(&job_id)
            .fetch_one(&ctx.db)
            .await
            .unwrap();
    assert_eq!(row.0, job_id);
    assert_eq!(row.1, "completed");
    assert_eq!(row.2, "Durable Job");
}

#[tokio::test]
async fn test_transfer_recursive_directory_copy() {
    let ctx = setup_app().await;
    let storage_dir = ctx.temp.path().join("storage");
    let src_dir = storage_dir.join("my_folder");
    let sub_dir = src_dir.join("sub_folder");
    std::fs::create_dir_all(&sub_dir).unwrap();
    std::fs::write(src_dir.join("file1.txt"), b"file 1 content").unwrap();
    std::fs::write(sub_dir.join("nested.txt"), b"nested file content").unwrap();

    let job_id = submit_transfer(
        &ctx.app,
        &ctx.cookie,
        "Copy my_folder to backup_folder",
        "copy",
        "/my_folder",
        "/backup_folder",
    )
    .await;
    wait_completed(&ctx.app, &ctx.cookie, &job_id).await;

    let dst_dir = storage_dir.join("backup_folder");
    assert!(dst_dir.is_dir());
    assert_eq!(std::fs::read(dst_dir.join("file1.txt")).unwrap(), b"file 1 content");
    assert_eq!(
        std::fs::read(dst_dir.join("sub_folder").join("nested.txt")).unwrap(),
        b"nested file content"
    );
}

#[tokio::test]
async fn test_transfer_persistent_clear_and_dismiss() {
    let ctx = setup_app().await;
    let job_id = submit_transfer(
        &ctx.app,
        &ctx.cookie,
        "Dismissable Transfer",
        "copy",
        "/source.txt",
        "/dest_dismiss.txt",
    )
    .await;
    wait_completed(&ctx.app, &ctx.cookie, &job_id).await;

    let clear_req = Request::builder()
        .uri("/api/v1/transfers/clear-finished")
        .method("POST")
        .header(header::COOKIE, &ctx.cookie)
        .body(Body::empty())
        .unwrap();
    let resp = ctx.app.clone().oneshot(clear_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(list_transfers(&ctx.app, &ctx.cookie).await.is_empty());

    let row: (Option<String>,) =
        sqlx::query_as("SELECT dismissed_at FROM transfer_jobs WHERE id = ?")
            .bind(&job_id)
            .fetch_one(&ctx.db)
            .await
            .unwrap();
    assert!(row.0.is_some());
}

#[tokio::test]
async fn test_transfer_user_ownership_authorization() {
    let ctx = setup_app().await;
    let hashed_pw = backend::auth::hash_password("secret123").unwrap();
    let bob_id = "user_bob_123";
    sqlx::query(
        "INSERT INTO users (id, username, password_hash, is_admin, created_at, updated_at)\n         VALUES (?, 'bob', ?, 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
    )
    .bind(bob_id)
    .bind(hashed_pw)
    .execute(&ctx.db)
    .await
    .unwrap();

    let login_req = Request::builder()
        .uri("/api/v1/auth/login")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "username": "bob", "password": "secret123" }).to_string(),
        ))
        .unwrap();
    let login_resp = ctx.app.clone().oneshot(login_req).await.unwrap();
    let bob_cookie = login_resp
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();

    let admin_job_id = submit_transfer(
        &ctx.app,
        &ctx.cookie,
        "Admin Job",
        "copy",
        "/source.txt",
        "/admin_dest.txt",
    )
    .await;

    let bob_cancel_req = Request::builder()
        .uri(format!("/api/v1/transfers/{admin_job_id}/cancel"))
        .method("POST")
        .header(header::COOKIE, &bob_cookie)
        .body(Body::empty())
        .unwrap();
    let cancel_resp = ctx.app.clone().oneshot(bob_cancel_req).await.unwrap();
    assert_eq!(cancel_resp.status(), StatusCode::FORBIDDEN);

    let bob_jobs = list_transfers(&ctx.app, &bob_cookie).await;
    assert!(bob_jobs.iter().all(|job| job.id != admin_job_id));
}

#[tokio::test]
async fn test_transfer_dynamic_limits_update() {
    let ctx = setup_app().await;
    let settings = SettingsState::from_ref(&ctx.state);
    let actor = Actor {
        id: "admin-dynamic-limit".to_string(),
        username: "admin".to_string(),
        is_admin: true,
    };
    let mut current = settings.service.get_settings(&actor).await.unwrap().settings;
    current.transfers.max_concurrent_transfers = 8;
    settings
        .service
        .update_settings(
            &actor,
            UpdateSettingsRequest {
                settings: Some(current),
                local_root: None,
                temp_dir: None,
                allow_symlinks: None,
                show_hidden_default: None,
                read_only_default: None,
            },
        )
        .await
        .unwrap();

    let job_id = submit_transfer(
        &ctx.app,
        &ctx.cookie,
        "Dynamic Limits Job",
        "copy",
        "/source.txt",
        "/dyn_dest.txt",
    )
    .await;
    wait_completed(&ctx.app, &ctx.cookie, &job_id).await;
}
