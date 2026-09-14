mod support;

use backend::{
    bootstrap::build_user_service,
    db::DbPool,
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
use support::{eventually_default, TestDatabase};
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
    let TestDatabase { pool: db, temp, .. } =
        TestDatabase::seeded("transfer_invariants.db").await;
    let storage_dir = temp.path().join("storage");
    std::fs::create_dir_all(&storage_dir).unwrap();
    std::fs::write(storage_dir.join("source.txt"), b"Invariant test file payload").unwrap();

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
        Arc::new(backend::runtime::ResourceBudget::default()),
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

#[test]
fn transfer_phase_string_roundtrip_is_stable() {
    let phases = [
        (TransferPhase::Preparing, "preparing"),
        (TransferPhase::Transferring, "transferring"),
        (TransferPhase::Finalizing, "finalizing"),
        (TransferPhase::Verifying, "verifying"),
        (TransferPhase::CleaningUp, "cleaning_up"),
        (TransferPhase::Completed, "completed"),
    ];

    for (phase, serialized) in phases {
        assert_eq!(phase.as_str(), serialized);
        assert_eq!(TransferPhase::from_str(serialized), phase);
    }
}

#[tokio::test]
async fn retry_revalidates_permissions_after_failure() {
    let fixture = setup_fixture().await;
    let users = build_user_service(fixture.db.clone());
    let alice_id = users
        .create_user("alice", "alicepassword", false)
        .await
        .unwrap();

    let mut job = failed_job("alice-failed", "local", "/source.txt");
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
    let result = control(&fixture).retry(&alice, "alice-failed").await;
    assert!(matches!(result, Err(backend::errors::AppError::Forbidden(_))));
}

#[tokio::test]
async fn retry_rejects_disabled_connection() {
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
        .insert_job_for_test(failed_job("disabled-connection", remote_id, "/file.txt"))
        .await;

    let result = control(&fixture)
        .retry(&admin_actor(), "disabled-connection")
        .await;
    assert!(matches!(result, Err(backend::errors::AppError::BadRequest(_))));
}

#[tokio::test]
async fn retry_rejects_unavailable_provider() {
    let fixture = setup_fixture().await;
    let job_id = "missing-provider";
    fixture
        .manager
        .insert_job_for_test(failed_job(job_id, "nonexistent-provider", "/missing.txt"))
        .await;

    match fixture.manager.retry_job(job_id, None, true).await {
        Err(RetryTransferError::ProviderUnavailable(message)) => {
            assert!(message.contains("nonexistent-provider"));
        }
        other => panic!("expected ProviderUnavailable, got {other:?}"),
    }
}

#[tokio::test]
async fn retry_rejects_missing_source() {
    let fixture = setup_fixture().await;
    let job_id = "missing-source";
    fixture
        .manager
        .insert_job_for_test(failed_job(job_id, "local", "/does_not_exist_anywhere.txt"))
        .await;

    match fixture.manager.retry_job(job_id, None, true).await {
        Err(RetryTransferError::SourceUnavailable(message)) => {
            assert!(message.contains("is not accessible"));
        }
        other => panic!("expected SourceUnavailable, got {other:?}"),
    }
}

#[tokio::test]
async fn move_retry_from_cleanup_resumes_cleanup_without_recopy() {
    let fixture = setup_fixture().await;
    let storage_dir = fixture.temp.path().join("storage");
    let move_src = storage_dir.join("move_to_clean.txt");
    let move_dst = storage_dir.join("move_cleaned_dest.txt");
    std::fs::write(&move_src, b"Move cleanup payload").unwrap();
    std::fs::write(&move_dst, b"Move cleanup payload already copied").unwrap();

    let job_id = "move-cleanup-recovery";
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

    let final_job = eventually_default("move retry to finish cleanup", || async {
        let job = fixture.manager.get_job(job_id).await?;
        (job.status == TransferStatus::Completed).then_some(job)
    })
    .await;
    assert_eq!(final_job.phase, TransferPhase::Completed);
    assert!(!move_src.exists());
    assert!(move_dst.exists());
}

#[tokio::test]
async fn concurrent_retry_compare_and_swap_admits_one_attempt() {
    let fixture = setup_fixture().await;
    let job_id = "concurrent-retry";
    fixture
        .manager
        .insert_job_for_test(failed_job(job_id, "local", "/source.txt"))
        .await;

    let first = fixture.manager.clone();
    let second = fixture.manager.clone();
    let first_task = tokio::spawn(async move { first.retry_job(job_id, None, true).await });
    let second_task = tokio::spawn(async move { second.retry_job(job_id, None, true).await });
    let (first_result, second_result) = tokio::join!(first_task, second_task);
    let first_result = first_result.unwrap();
    let second_result = second_result.unwrap();
    assert_eq!(
        usize::from(first_result.is_ok()) + usize::from(second_result.is_ok()),
        1
    );

    let error = if let Err(error) = first_result {
        error
    } else {
        second_result.unwrap_err()
    };
    assert!(matches!(error, RetryTransferError::InvalidStatus(_, _)));
}

#[tokio::test]
async fn dismissed_terminal_job_cannot_be_retried() {
    let fixture = setup_fixture().await;
    let job_id = "dismissed-job";
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
async fn db_fallback_rejects_cancellation_after_finalizing_begins() {
    let fixture = setup_fixture().await;
    let job_id = "sqlite-finalizing-job";
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
async fn phase_change_forces_progress_checkpoint_persistence() {
    let fixture = setup_fixture().await;
    let job_id = "checkpoint-persistence";
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
async fn terminal_inline_completion_is_reloaded_from_durable_history_with_payload_parity() {
    let fixture = setup_fixture().await;
    let job_id = "fast-transfer-parity";
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

    let json_response = serde_json::to_value(final_job.to_response()).unwrap();
    assert_eq!(json_response["id"], job_id);
    assert_eq!(json_response["transferred_bytes"], 42);
    assert_eq!(json_response["capabilities"]["can_cancel"], false);

    let event = DomainEvent::transfer_completed(&final_job);
    let payload = match event {
        DomainEvent::TransferCompleted(value) => value,
        _ => unreachable!(),
    };
    assert_eq!(payload["id"], job_id);
    assert_eq!(payload["capabilities"]["can_cancel"], false);
}
