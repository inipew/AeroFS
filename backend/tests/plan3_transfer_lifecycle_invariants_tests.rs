use backend::{
    db::{init_db, DbPool},
    domain::Actor,
    events::{DomainEvent, EventJournal},
    infrastructure::transfers::SqliteTransferControl,
    ports::transfer::TransferControl,
    services::UploadLockManager,
    transfer::{
        RetryTransferError, TransferExecutionMode, TransferJob, TransferManager, TransferPhase,
        TransferStatus, TransferType,
    },
    vfs::{factory::ProviderFactory, registry::ProviderRegistry},
};
use chrono::Utc;
use std::sync::Arc;
use tempfile::tempdir;
use tokio_util::{sync::CancellationToken, task::TaskTracker};

struct TransferFixture {
    manager: TransferManager,
    db: DbPool,
    temp: tempfile::TempDir,
    shutdown: CancellationToken,
    tracker: TaskTracker,
}

impl Drop for TransferFixture {
    fn drop(&mut self) {
        self.shutdown.cancel();
        self.tracker.close();
    }
}

async fn setup_fixture() -> TransferFixture {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("transfer_invariants.db");
    let storage_dir = temp.path().join("storage");
    std::fs::create_dir_all(&storage_dir).unwrap();
    std::fs::write(storage_dir.join("source.txt"), b"Invariant test file payload").unwrap();

    let database_url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    let db = init_db(&database_url).await.unwrap();
    let journal = Arc::new(EventJournal::init(db.clone()).await.unwrap());
    let registry = ProviderRegistry::new();
    let local = ProviderFactory::build_local("local", storage_dir).unwrap();
    registry.register("local".to_string(), local).await;

    let shutdown = CancellationToken::new();
    let tracker = TaskTracker::new();
    let manager = TransferManager::new(
        registry.providers_map(),
        db.clone(),
        4,
        journal,
        shutdown.clone(),
        &tracker,
    )
    .await;

    TransferFixture {
        manager,
        db,
        temp,
        shutdown,
        tracker,
    }
}

fn failed_job(id: &str, source_connection: &str, source_path: &str) -> TransferJob {
    let now = Utc::now();
    TransferJob {
        id: id.into(),
        user_id: None,
        name: format!("{id}.bin"),
        transfer_type: TransferType::Copy,
        source_connection_id: source_connection.into(),
        source_path: source_path.into(),
        destination_connection_id: "local".into(),
        destination_path: format!("/{id}-dest.bin"),
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
    }
}

fn admin_actor() -> Actor {
    Actor {
        id: "admin-test".into(),
        username: "admin".into(),
        is_admin: true,
    }
}

fn control(fixture: &TransferFixture) -> SqliteTransferControl {
    SqliteTransferControl::new(
        fixture.db.clone(),
        fixture.manager.clone(),
        Arc::new(UploadLockManager::default()),
    )
}

#[tokio::test]
async fn test_retry_rejected_after_permission_revoked() {
    let fixture = setup_fixture().await;
    let alice_id = backend::services::UserService::create_user(
        &fixture.db,
        "alice",
        "alicepassword",
        false,
    )
    .await
    .unwrap();

    let mut job = failed_job("test-job-alice-failed", "local", "/source.txt");
    job.user_id = Some(alice_id.clone());
    fixture.manager.insert_job_for_test(job).await;

    let perm_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO permissions (id, user_id, connection_id, can_read, can_write, can_create, can_delete, can_rename, can_upload, can_download)\n         VALUES (?, ?, 'local', 0, 1, 1, 1, 1, 1, 1)\n         ON CONFLICT(user_id, connection_id) DO UPDATE SET can_read = 0",
    )
    .bind(perm_id)
    .bind(&alice_id)
    .execute(&fixture.db)
    .await
    .unwrap();

    let alice = Actor {
        id: alice_id,
        username: "alice".into(),
        is_admin: false,
    };
    let result = control(&fixture)
        .retry(&alice, "test-job-alice-failed")
        .await;
    assert!(matches!(result, Err(backend::errors::AppError::Forbidden(_))));
}

#[tokio::test]
async fn test_retry_rejected_when_connection_disabled() {
    let fixture = setup_fixture().await;
    let remote_id = "disabled-remote-conn";
    sqlx::query(
        "INSERT INTO connections (id, name, provider, base_path, read_only, enabled, created_at, updated_at)\n         VALUES (?, 'Disabled Remote', 'local', '/', 0, 0, datetime('now'), datetime('now'))",
    )
    .bind(remote_id)
    .execute(&fixture.db)
    .await
    .unwrap();

    fixture
        .manager
        .insert_job_for_test(failed_job(
            "test-job-disabled-conn",
            remote_id,
            "/file.txt",
        ))
        .await;

    let result = control(&fixture)
        .retry(&admin_actor(), "test-job-disabled-conn")
        .await;
    assert!(matches!(result, Err(backend::errors::AppError::BadRequest(_))));
}

#[tokio::test]
async fn test_retry_rejected_when_provider_unavailable() {
    let fixture = setup_fixture().await;
    let job_id = "test-job-missing-provider";
    fixture
        .manager
        .insert_job_for_test(failed_job(job_id, "nonexistent-provider", "/missing.txt"))
        .await;

    match fixture.manager.retry_job(job_id, None, true).await {
        Err(RetryTransferError::ProviderUnavailable(msg)) => {
            assert!(msg.contains("nonexistent-provider"));
        }
        other => panic!("Expected ProviderUnavailable, got {other:?}"),
    }
}

#[tokio::test]
async fn test_retry_rejected_when_source_missing() {
    let fixture = setup_fixture().await;
    let job_id = "test-job-missing-source-file";
    fixture
        .manager
        .insert_job_for_test(failed_job(
            job_id,
            "local",
            "/does_not_exist_anywhere.txt",
        ))
        .await;

    match fixture.manager.retry_job(job_id, None, true).await {
        Err(RetryTransferError::SourceUnavailable(msg)) => {
            assert!(msg.contains("is not accessible"));
        }
        other => panic!("Expected SourceUnavailable, got {other:?}"),
    }
}

#[tokio::test]
async fn test_retry_move_recovery_from_cleaning_up_without_recopy() {
    let fixture = setup_fixture().await;
    let storage_dir = fixture.temp.path().join("storage");
    let move_src = storage_dir.join("move_to_clean.txt");
    let move_dst = storage_dir.join("move_cleaned_dest.txt");
    std::fs::write(&move_src, b"Move cleanup payload").unwrap();
    std::fs::write(&move_dst, b"Move cleanup payload already copied").unwrap();

    let job_id = "test-move-cleaning-up-recovery";
    let mut job = failed_job(job_id, "local", "/move_to_clean.txt");
    job.transfer_type = TransferType::Move;
    job.destination_path = "/move_cleaned_dest.txt".into();
    job.phase = TransferPhase::CleaningUp;
    job.transferred_bytes = 20;
    job.total_bytes = 20;
    fixture.manager.insert_job_for_test(job).await;

    fixture.manager.retry_job(job_id, None, true).await.unwrap();
    let queued = fixture.manager.get_job(job_id).await.unwrap();
    assert_eq!(queued.status, TransferStatus::Queued);
    assert_eq!(queued.phase, TransferPhase::CleaningUp);

    for _ in 0..30 {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        if fixture.manager.get_job(job_id).await.unwrap().status == TransferStatus::Completed {
            break;
        }
    }
    let final_job = fixture.manager.get_job(job_id).await.unwrap();
    assert_eq!(final_job.status, TransferStatus::Completed);
    assert_eq!(final_job.phase, TransferPhase::Completed);
    assert!(!move_src.exists());
    assert!(move_dst.exists());
}

#[tokio::test]
async fn test_concurrent_retry_race_condition_cas_guard() {
    let fixture = setup_fixture().await;
    let job_id = "test-concurrent-retry-job";
    fixture
        .manager
        .insert_job_for_test(failed_job(job_id, "local", "/source.txt"))
        .await;

    let tm1 = fixture.manager.clone();
    let tm2 = fixture.manager.clone();
    let t1 = tokio::spawn(async move { tm1.retry_job(job_id, None, true).await });
    let t2 = tokio::spawn(async move { tm2.retry_job(job_id, None, true).await });
    let (r1, r2) = tokio::join!(t1, t2);
    let r1 = r1.unwrap();
    let r2 = r2.unwrap();
    let success_count = usize::from(r1.is_ok()) + usize::from(r2.is_ok());
    assert_eq!(success_count, 1);

    let error = if let Err(error) = r1 {
        error
    } else {
        r2.unwrap_err()
    };
    assert!(matches!(error, RetryTransferError::InvalidStatus(_, _)));
}

#[tokio::test]
async fn test_retry_dismissed_job_rejected() {
    let fixture = setup_fixture().await;
    let job_id = "test-dismissed-job";
    let mut job = failed_job(job_id, "local", "/source.txt");
    job.dismissed_at = Some(Utc::now());
    fixture.manager.insert_job_for_test(job).await;

    assert!(matches!(
        fixture.manager.retry_job(job_id, None, true).await,
        Err(RetryTransferError::Dismissed(id)) if id == job_id
    ));
    assert!(matches!(
        control(&fixture).retry(&admin_actor(), job_id).await,
        Err(backend::errors::AppError::BadRequest(_))
    ));
}

#[tokio::test]
async fn test_cancellation_db_fallback_terminal_or_finalizing() {
    let fixture = setup_fixture().await;
    let job_id = "test-sqlite-finalizing-job";
    let now = Utc::now();
    let job = TransferJob {
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
    sqlx::query(
        "INSERT INTO transfer_jobs (id, user_id, name, transfer_type, source_connection_id, source_path, destination_connection_id, destination_path, status, phase, transferred_bytes, total_bytes, speed_bytes_per_sec, created_at, updated_at)\n         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&job.id)
    .bind(&job.user_id)
    .bind(&job.name)
    .bind(job.transfer_type.as_str())
    .bind(&job.source_connection_id)
    .bind(&job.source_path)
    .bind(&job.destination_connection_id)
    .bind(&job.destination_path)
    .bind(job.status.as_str())
    .bind(job.phase.as_str())
    .bind(job.transferred_bytes as i64)
    .bind(job.total_bytes as i64)
    .bind(job.speed_bytes_per_sec as i64)
    .bind(job.created_at.to_rfc3339())
    .bind(job.updated_at.to_rfc3339())
    .execute(&fixture.db)
    .await
    .unwrap();

    let result = control(&fixture).cancel(&admin_actor(), job_id).await;
    assert!(matches!(result, Err(backend::errors::AppError::Conflict(_))));
}

#[tokio::test]
async fn test_persistence_checkpoint_throttling_on_phase_and_status() {
    let fixture = setup_fixture().await;
    let job_id = "test-checkpoint-persistence";
    let now = Utc::now();
    let job = TransferJob {
        id: job_id.into(),
        user_id: None,
        name: "checkpoint.txt".into(),
        transfer_type: TransferType::Upload,
        source_connection_id: "upload".into(),
        source_path: format!("upload://{job_id}"),
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
    fixture.manager.insert_job_for_test(job).await;
    fixture
        .manager
        .update_inline_progress(job_id, 100, 10_000_000, 100, None)
        .await;
    let _ = fixture.manager.try_enter_finalizing(job_id).await;

    let db_job: (String,) = sqlx::query_as("SELECT phase FROM transfer_jobs WHERE id = ?")
        .bind(job_id)
        .fetch_one(&fixture.db)
        .await
        .unwrap();
    assert_eq!(db_job.0, "finalizing");
}

#[tokio::test]
async fn test_fast_transfer_final_progress_and_payload_parity() {
    let fixture = setup_fixture().await;
    let job_id = "test-fast-transfer-parity";
    let now = Utc::now();
    let job = TransferJob {
        id: job_id.into(),
        user_id: None,
        name: "fast.bin".into(),
        transfer_type: TransferType::Upload,
        source_connection_id: "upload".into(),
        source_path: format!("upload://{job_id}"),
        destination_connection_id: "local".into(),
        destination_path: "/fast.bin".into(),
        status: TransferStatus::Running,
        phase: TransferPhase::Transferring,
        execution_mode: TransferExecutionMode::Inline,
        staging: Default::default(),
        transferred_bytes: 0,
        total_bytes: 0,
        speed_bytes_per_sec: 0,
        eta_seconds: None,
        checksum: None,
        error_message: None,
        dismissed_at: None,
        created_at: now,
        updated_at: now,
    };
    fixture.manager.insert_job_for_test(job).await;
    fixture
        .manager
        .update_inline_progress(job_id, 42, 42, 0, Some(0))
        .await;
    fixture
        .manager
        .complete_inline_job(job_id, Some("sha256:abc".into()))
        .await;

    let final_job = fixture.manager.get_job(job_id).await.unwrap();
    assert_eq!(final_job.status, TransferStatus::Completed);
    assert_eq!(final_job.phase, TransferPhase::Completed);
    assert_eq!(final_job.transferred_bytes, 42);
    assert_eq!(final_job.total_bytes, 42);

    let json_resp = serde_json::to_value(final_job.to_response()).unwrap();
    assert_eq!(json_resp["id"], job_id);
    assert_eq!(json_resp["transferred_bytes"], 42);
    assert_eq!(json_resp["capabilities"]["can_cancel"], false);

    let event = DomainEvent::transfer_completed(&final_job);
    let payload = match event {
        DomainEvent::TransferCompleted(value) => value,
        _ => unreachable!(),
    };
    assert_eq!(payload["id"], job_id);
    assert_eq!(payload["capabilities"]["can_cancel"], false);
}
