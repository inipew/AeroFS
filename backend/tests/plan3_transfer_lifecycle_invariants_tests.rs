use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use backend::{
    config::AppConfig,
    create_router,
    db::init_db,
    events::DomainEvent,
    transfer::{
        RetryTransferError, TransferExecutionMode, TransferJob, TransferPhase, TransferStatus,
        TransferType,
    },
    AppState,
};
use chrono::Utc;
use serde_json::json;
use tempfile::tempdir;
use tower::ServiceExt;

async fn setup_app() -> (axum::Router, String, tempfile::TempDir, AppState) {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("transfer_invariants.db");
    let storage_dir = temp.path().join("storage");
    std::fs::create_dir_all(&storage_dir).unwrap();

    let source_file = storage_dir.join("source.txt");
    std::fs::write(&source_file, b"Invariant test file payload").unwrap();

    let mut config = AppConfig::default();
    config.database.url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    config.filesystem.default_local_root = storage_dir;

    let db = init_db(&config.database.url).await.unwrap();
    let state = AppState::new_with_db(config, db).await;
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
    let cookie = resp
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();

    (app, cookie, temp, state)
}

#[tokio::test]
async fn test_retry_rejected_after_permission_revoked() {
    let (app, _admin_cookie, _temp, state) = setup_app().await;

    // 1. Create a non-admin user "alice"
    let alice_id = backend::services::UserService::create_user(
        &state.db,
        "alice",
        "alicepassword",
        false,
    )
    .await
    .unwrap();

    let login_req = Request::builder()
        .uri("/api/v1/auth/login")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "username": "alice", "password": "alicepassword" }).to_string(),
        ))
        .unwrap();
    let login_resp = app.clone().oneshot(login_req).await.unwrap();
    let alice_cookie = login_resp
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();

    // 2. Insert a failed transfer owned by Alice
    let job_id = "test-job-alice-failed";
    let now = Utc::now();
    let job = TransferJob {
        id: job_id.into(),
        user_id: Some(alice_id.clone()),
        name: "alice_file.txt".into(),
        transfer_type: TransferType::Copy,
        source_connection_id: "local".into(),
        source_path: "/source.txt".into(),
        destination_connection_id: "local".into(),
        destination_path: "/dst_alice.txt".into(),
        status: TransferStatus::Failed,
        phase: TransferPhase::Preparing,
        execution_mode: TransferExecutionMode::Background,
        staging: Default::default(),
        transferred_bytes: 0,
        total_bytes: 100,
        speed_bytes_per_sec: 0,
        eta_seconds: None,
        checksum: None,
        error_message: Some("network error".into()),
        dismissed_at: None,
        created_at: now,
        updated_at: now,
    };
    state.transfer_manager.insert_job_for_test(job.clone()).await;

    // 3. Set Alice's read permission on source connection "local" to 0
    let perm_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO permissions (id, user_id, connection_id, can_read, can_write, can_create, can_delete, can_rename, can_upload, can_download)
         VALUES (?, ?, 'local', 0, 1, 1, 1, 1, 1, 1)
         ON CONFLICT(user_id, connection_id) DO UPDATE SET can_read = 0",
    )
    .bind(perm_id)
    .bind(&alice_id)
    .execute(&state.db)
    .await
    .unwrap();

    // 4. Alice attempts to retry -> MUST return 403 Forbidden
    let retry_req = Request::builder()
        .uri(format!("/api/v1/transfers/{}/retry", job_id))
        .method("POST")
        .header(header::COOKIE, &alice_cookie)
        .body(Body::empty())
        .unwrap();

    let retry_resp = app.clone().oneshot(retry_req).await.unwrap();
    assert_eq!(retry_resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_retry_rejected_when_connection_disabled() {
    let (app, admin_cookie, _temp, state) = setup_app().await;

    // Insert a remote connection marked as disabled (enabled = 0)
    let remote_id = "disabled-remote-conn";
    sqlx::query(
        "INSERT INTO connections (id, name, provider, base_path, read_only, enabled, created_at, updated_at)
         VALUES (?, 'Disabled Remote', 'local', '/', 0, 0, datetime('now'), datetime('now'))",
    )
    .bind(remote_id)
    .execute(&state.db)
    .await
    .unwrap();

    let job_id = "test-job-disabled-conn";
    let now = Utc::now();
    let job = TransferJob {
        id: job_id.into(),
        user_id: None,
        name: "remote_file.txt".into(),
        transfer_type: TransferType::Copy,
        source_connection_id: remote_id.into(),
        source_path: "/file.txt".into(),
        destination_connection_id: "local".into(),
        destination_path: "/dest.txt".into(),
        status: TransferStatus::Failed,
        phase: TransferPhase::Preparing,
        execution_mode: TransferExecutionMode::Background,
        staging: Default::default(),
        transferred_bytes: 0,
        total_bytes: 100,
        speed_bytes_per_sec: 0,
        eta_seconds: None,
        checksum: None,
        error_message: Some("failed".into()),
        dismissed_at: None,
        created_at: now,
        updated_at: now,
    };
    state.transfer_manager.insert_job_for_test(job).await;

    let retry_req = Request::builder()
        .uri(format!("/api/v1/transfers/{}/retry", job_id))
        .method("POST")
        .header(header::COOKIE, &admin_cookie)
        .body(Body::empty())
        .unwrap();

    let retry_resp = app.clone().oneshot(retry_req).await.unwrap();
    assert_eq!(retry_resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_retry_rejected_when_provider_unavailable() {
    let (_app, _admin_cookie, _temp, state) = setup_app().await;

    let job_id = "test-job-missing-provider";
    let now = Utc::now();
    let job = TransferJob {
        id: job_id.into(),
        user_id: None,
        name: "missing.txt".into(),
        transfer_type: TransferType::Copy,
        source_connection_id: "nonexistent-provider".into(),
        source_path: "/missing.txt".into(),
        destination_connection_id: "local".into(),
        destination_path: "/dest.txt".into(),
        status: TransferStatus::Failed,
        phase: TransferPhase::Preparing,
        execution_mode: TransferExecutionMode::Background,
        staging: Default::default(),
        transferred_bytes: 0,
        total_bytes: 100,
        speed_bytes_per_sec: 0,
        eta_seconds: None,
        checksum: None,
        error_message: Some("provider lost".into()),
        dismissed_at: None,
        created_at: now,
        updated_at: now,
    };
    state.transfer_manager.insert_job_for_test(job).await;

    let res = state.transfer_manager.retry_job(job_id, None, true).await;
    match res {
        Err(RetryTransferError::ProviderUnavailable(msg)) => {
            assert!(msg.contains("nonexistent-provider"));
        }
        other => panic!("Expected ProviderUnavailable, got {:?}", other),
    }
}

#[tokio::test]
async fn test_retry_rejected_when_source_missing() {
    let (_app, _admin_cookie, _temp, state) = setup_app().await;

    let job_id = "test-job-missing-source-file";
    let now = Utc::now();
    let job = TransferJob {
        id: job_id.into(),
        user_id: None,
        name: "ghost.txt".into(),
        transfer_type: TransferType::Copy,
        source_connection_id: "local".into(),
        source_path: "/does_not_exist_anywhere.txt".into(),
        destination_connection_id: "local".into(),
        destination_path: "/dest.txt".into(),
        status: TransferStatus::Failed,
        phase: TransferPhase::Preparing,
        execution_mode: TransferExecutionMode::Background,
        staging: Default::default(),
        transferred_bytes: 0,
        total_bytes: 100,
        speed_bytes_per_sec: 0,
        eta_seconds: None,
        checksum: None,
        error_message: Some("failed".into()),
        dismissed_at: None,
        created_at: now,
        updated_at: now,
    };
    state.transfer_manager.insert_job_for_test(job).await;

    let res = state.transfer_manager.retry_job(job_id, None, true).await;
    match res {
        Err(RetryTransferError::SourceUnavailable(msg)) => {
            assert!(msg.contains("is not accessible"));
        }
        other => panic!("Expected SourceUnavailable, got {:?}", other),
    }
}

#[tokio::test]
async fn test_retry_move_recovery_from_cleaning_up_without_recopy() {
    let (_app, _admin_cookie, temp, state) = setup_app().await;

    let storage_dir = temp.path().join("storage");
    let move_src = storage_dir.join("move_to_clean.txt");
    let move_dst = storage_dir.join("move_cleaned_dest.txt");
    std::fs::write(&move_src, b"Move cleanup payload").unwrap();
    std::fs::write(&move_dst, b"Move cleanup payload already copied").unwrap();

    let job_id = "test-move-cleaning-up-recovery";
    let now = Utc::now();
    let job = TransferJob {
        id: job_id.into(),
        user_id: None,
        name: "move_to_clean.txt".into(),
        transfer_type: TransferType::Move,
        source_connection_id: "local".into(),
        source_path: "/move_to_clean.txt".into(),
        destination_connection_id: "local".into(),
        destination_path: "/move_cleaned_dest.txt".into(),
        status: TransferStatus::Failed,
        phase: TransferPhase::CleaningUp,
        execution_mode: TransferExecutionMode::Background,
        staging: Default::default(),
        transferred_bytes: 20,
        total_bytes: 20,
        speed_bytes_per_sec: 0,
        eta_seconds: None,
        checksum: None,
        error_message: Some("failed deleting source".into()),
        dismissed_at: None,
        created_at: now,
        updated_at: now,
    };
    state.transfer_manager.insert_job_for_test(job).await;

    // Retry job: phase must be preserved as CleaningUp
    let retry_res = state.transfer_manager.retry_job(job_id, None, true).await;
    assert!(retry_res.is_ok(), "Retry should succeed");

    let queued_job = state.transfer_manager.get_job(job_id).await.unwrap();
    assert_eq!(queued_job.status, TransferStatus::Queued);
    assert_eq!(queued_job.phase, TransferPhase::CleaningUp);

    // Wait for worker to complete cleanup
    for _ in 0..30 {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let j = state.transfer_manager.get_job(job_id).await.unwrap();
        if j.status == TransferStatus::Completed {
            break;
        }
    }

    let final_job = state.transfer_manager.get_job(job_id).await.unwrap();
    assert_eq!(final_job.status, TransferStatus::Completed);
    assert_eq!(final_job.phase, TransferPhase::Completed);
    // Source file should have been deleted without destination conflict
    assert!(!move_src.exists(), "Source file should have been deleted during CleaningUp");
    assert!(move_dst.exists(), "Destination file must still exist");
}

#[tokio::test]
async fn test_concurrent_retry_race_condition_cas_guard() {
    let (_app, _admin_cookie, _temp, state) = setup_app().await;

    let job_id = "test-concurrent-retry-job";
    let now = Utc::now();
    let job = TransferJob {
        id: job_id.into(),
        user_id: None,
        name: "concurrent.txt".into(),
        transfer_type: TransferType::Copy,
        source_connection_id: "local".into(),
        source_path: "/source.txt".into(),
        destination_connection_id: "local".into(),
        destination_path: "/concurrent_dst.txt".into(),
        status: TransferStatus::Failed,
        phase: TransferPhase::Preparing,
        execution_mode: TransferExecutionMode::Background,
        staging: Default::default(),
        transferred_bytes: 0,
        total_bytes: 100,
        speed_bytes_per_sec: 0,
        eta_seconds: None,
        checksum: None,
        error_message: Some("initial failure".into()),
        dismissed_at: None,
        created_at: now,
        updated_at: now,
    };
    state.transfer_manager.insert_job_for_test(job).await;

    // Launch two concurrent retries simultaneously
    let tm1 = state.transfer_manager.clone();
    let tm2 = state.transfer_manager.clone();
    let t1 = tokio::spawn(async move { tm1.retry_job(job_id, None, true).await });
    let t2 = tokio::spawn(async move { tm2.retry_job(job_id, None, true).await });

    let (res1, res2) = tokio::join!(t1, t2);
    let r1 = res1.unwrap();
    let r2 = res2.unwrap();

    let success_count = (if r1.is_ok() { 1 } else { 0 }) + (if r2.is_ok() { 1 } else { 0 });
    assert_eq!(success_count, 1, "Exactly one concurrent retry request must succeed");

    let err = match r1 {
        Err(e) => e,
        Ok(_) => r2.unwrap_err(),
    };
    match err {
        RetryTransferError::InvalidStatus(_, msg) => {
            assert!(
                msg.contains("transfer state changed while retry was being validated")
                    || msg.contains("cannot be retried"),
                "Second concurrent request should fail CAS status verification"
            );
        }
        other => panic!("Expected InvalidStatus on concurrent retry, got {:?}", other),
    }
}

#[tokio::test]
async fn test_retry_dismissed_job_rejected() {
    let (app, admin_cookie, _temp, state) = setup_app().await;

    let job_id = "test-dismissed-job";
    let now = Utc::now();
    let job = TransferJob {
        id: job_id.into(),
        user_id: None,
        name: "dismissed.txt".into(),
        transfer_type: TransferType::Copy,
        source_connection_id: "local".into(),
        source_path: "/source.txt".into(),
        destination_connection_id: "local".into(),
        destination_path: "/dismissed_dst.txt".into(),
        status: TransferStatus::Failed,
        phase: TransferPhase::Preparing,
        execution_mode: TransferExecutionMode::Background,
        staging: Default::default(),
        transferred_bytes: 0,
        total_bytes: 100,
        speed_bytes_per_sec: 0,
        eta_seconds: None,
        checksum: None,
        error_message: Some("initial failure".into()),
        dismissed_at: Some(now),
        created_at: now,
        updated_at: now,
    };
    state.transfer_manager.insert_job_for_test(job).await;

    // Direct engine retry call -> Err(Dismissed)
    let res = state.transfer_manager.retry_job(job_id, None, true).await;
    match res {
        Err(RetryTransferError::Dismissed(id)) => assert_eq!(id, job_id),
        other => panic!("Expected RetryTransferError::Dismissed, got {:?}", other),
    }

    // REST API retry call -> 400 Bad Request
    let retry_req = Request::builder()
        .uri(format!("/api/v1/transfers/{}/retry", job_id))
        .method("POST")
        .header(header::COOKIE, &admin_cookie)
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(retry_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_cancellation_db_fallback_terminal_or_finalizing() {
    let (app, admin_cookie, _temp, state) = setup_app().await;

    // 1. Insert a job into SQLite directly that is already Finalizing
    let job_id = "test-sqlite-finalizing-job";
    let now = Utc::now();
    let finalizing_job = TransferJob {
        id: job_id.into(),
        user_id: None,
        name: "finalizing.txt".into(),
        transfer_type: TransferType::Copy,
        source_connection_id: "local".into(),
        source_path: "/source.txt".into(),
        destination_connection_id: "local".into(),
        destination_path: "/finalizing_dst.txt".into(),
        status: TransferStatus::Running,
        phase: TransferPhase::Finalizing,
        execution_mode: TransferExecutionMode::Background,
        staging: Default::default(),
        transferred_bytes: 100,
        total_bytes: 100,
        speed_bytes_per_sec: 0,
        eta_seconds: None,
        checksum: None,
        error_message: None,
        dismissed_at: None,
        created_at: now,
        updated_at: now,
    };
    // Save to SQLite only (not in RAM)
    sqlx::query(
        "INSERT INTO transfer_jobs (id, user_id, name, transfer_type, source_connection_id, source_path, destination_connection_id, destination_path, status, phase, transferred_bytes, total_bytes, speed_bytes_per_sec, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&finalizing_job.id)
    .bind(&finalizing_job.user_id)
    .bind(&finalizing_job.name)
    .bind(finalizing_job.transfer_type.as_str())
    .bind(&finalizing_job.source_connection_id)
    .bind(&finalizing_job.source_path)
    .bind(&finalizing_job.destination_connection_id)
    .bind(&finalizing_job.destination_path)
    .bind(finalizing_job.status.as_str())
    .bind(finalizing_job.phase.as_str())
    .bind(finalizing_job.transferred_bytes as i64)
    .bind(finalizing_job.total_bytes as i64)
    .bind(finalizing_job.speed_bytes_per_sec as i64)
    .bind(finalizing_job.created_at.to_rfc3339())
    .bind(finalizing_job.updated_at.to_rfc3339())
    .execute(&state.db)
    .await
    .unwrap();

    // Cancellation must fail with 409 Conflict
    let cancel_req = Request::builder()
        .uri(format!("/api/v1/transfers/{}/cancel", job_id))
        .method("POST")
        .header(header::COOKIE, &admin_cookie)
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(cancel_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_persistence_checkpoint_throttling_on_phase_and_status() {
    let (_app, _admin_cookie, _temp, state) = setup_app().await;

    let job_id = "test-checkpoint-persistence";
    let now = Utc::now();
    let job = TransferJob {
        id: job_id.into(),
        user_id: None,
        name: "checkpoint.txt".into(),
        transfer_type: TransferType::Upload,
        source_connection_id: "upload".into(),
        source_path: format!("upload://{}", job_id),
        destination_connection_id: "local".into(),
        destination_path: "/checkpoint.txt".into(),
        status: TransferStatus::Running,
        phase: TransferPhase::Preparing,
        execution_mode: TransferExecutionMode::Inline,
        staging: Default::default(),
        transferred_bytes: 0,
        total_bytes: 10_000_000,
        speed_bytes_per_sec: 0,
        eta_seconds: None,
        checksum: None,
        error_message: None,
        dismissed_at: None,
        created_at: now,
        updated_at: now,
    };
    state.transfer_manager.insert_job_for_test(job).await;

    // Small progress update without phase change -> throttled
    state
        .transfer_manager
        .update_inline_progress(job_id, 100, 10_000_000, 100, None)
        .await;

    // Phase transition -> MUST persist immediately
    let _ = state.transfer_manager.try_enter_finalizing(job_id).await;

    let db_job: (String,) = sqlx::query_as("SELECT phase FROM transfer_jobs WHERE id = ?")
        .bind(job_id)
        .fetch_one(&state.db)
        .await
        .unwrap();

    assert_eq!(db_job.0, "finalizing");
}

#[tokio::test]
async fn test_fast_transfer_final_progress_and_payload_parity() {
    let (_app, _admin_cookie, _temp, state) = setup_app().await;

    let job_id = "test-fast-transfer-parity";
    let now = Utc::now();
    let job = TransferJob {
        id: job_id.into(),
        user_id: None,
        name: "fast.bin".into(),
        transfer_type: TransferType::Upload,
        source_connection_id: "upload".into(),
        source_path: format!("upload://{}", job_id),
        destination_connection_id: "local".into(),
        destination_path: "/fast.bin".into(),
        status: TransferStatus::Running,
        phase: TransferPhase::Transferring,
        execution_mode: TransferExecutionMode::Inline,
        staging: Default::default(),
        transferred_bytes: 0,
        total_bytes: 0, // unknown initial size
        speed_bytes_per_sec: 0,
        eta_seconds: None,
        checksum: None,
        error_message: None,
        dismissed_at: None,
        created_at: now,
        updated_at: now,
    };
    state.transfer_manager.insert_job_for_test(job.clone()).await;

    // Fast finish (< 100ms) with actual 42 bytes
    let actual_uploaded = 42;
    state
        .transfer_manager
        .update_inline_progress(job_id, actual_uploaded, actual_uploaded, 0, Some(0))
        .await;
    state.transfer_manager.complete_inline_job(job_id, Some("sha256:abc".into())).await;

    let final_job = state.transfer_manager.get_job(job_id).await.unwrap();
    assert_eq!(final_job.status, TransferStatus::Completed);
    assert_eq!(final_job.phase, TransferPhase::Completed);
    assert_eq!(final_job.transferred_bytes, 42);
    assert_eq!(final_job.total_bytes, 42);

    // Parity check: DTO to_response() flattened capabilities
    let resp = final_job.to_response();
    let json_resp = serde_json::to_value(&resp).unwrap();
    assert_eq!(json_resp["id"], job_id);
    assert_eq!(json_resp["transferred_bytes"], 42);
    assert_eq!(json_resp["total_bytes"], 42);
    assert_eq!(json_resp["capabilities"]["can_cancel"], false);
    assert_eq!(json_resp["capabilities"]["can_retry"], false);

    // WebSocket event parity: payload must have same JSON structure
    let event = DomainEvent::transfer_completed(&final_job);
    let event_payload = match event {
        DomainEvent::TransferCompleted(j) => j,
        _ => panic!("Expected TransferCompleted payload"),
    };
    assert_eq!(event_payload["id"], job_id);
    assert_eq!(event_payload["capabilities"]["can_cancel"], false);
}
