use crate::support::{eventually_default, TestDatabase};
use backend::{
    db::DbPool,
    events::EventJournal,
    runtime::{ResourceBudget, TaskSupervisor},
    sync::{scanner::SCAN_BATCH_SIZE, SyncManager, SyncStatus, StreamingSyncPlan, SyncStrategy},
    transfer::{
        TransferExecutionMode, TransferJob, TransferManager, TransferPhase, TransferStatus,
        TransferType,
    },
    vfs::{factory::ProviderFactory, registry::ProviderRegistry, FileSystem},
};
use chrono::Utc;
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio_util::{sync::CancellationToken, task::TaskTracker};

struct SyncRuntimeFixture {
    manager: SyncManager,
    transfer_manager: TransferManager,
    db: DbPool,
    temp: tempfile::TempDir,
    storage_root: PathBuf,
    provider: Arc<dyn FileSystem>,
    shutdown: CancellationToken,
    tracker: TaskTracker,
    supervisor: TaskSupervisor,
}

impl Drop for SyncRuntimeFixture {
    fn drop(&mut self) {
        self.shutdown.cancel();
        self.tracker.close();
    }
}

async fn setup_fixture() -> SyncRuntimeFixture {
    let TestDatabase { pool: db, temp, .. } = TestDatabase::migrated("sync_recovery.db").await;
    let storage_root = temp.path().join("storage");
    std::fs::create_dir_all(&storage_root).unwrap();

    let journal = Arc::new(EventJournal::init(db.clone()).await.unwrap());
    let registry = ProviderRegistry::new();
    let provider = ProviderFactory::build_local("local", storage_root.clone()).unwrap();
    registry
        .register("local".to_string(), Arc::clone(&provider))
        .await;

    let shutdown = CancellationToken::new();
    let tracker = TaskTracker::new();
    let budget = Arc::new(ResourceBudget::default());
    let transfer_manager = TransferManager::new(
        registry.providers_map(),
        db.clone(),
        4,
        Arc::clone(&budget),
        Arc::clone(&journal),
        shutdown.clone(),
        &tracker,
    )
    .await;
    let supervisor = TaskSupervisor::new();
    let manager = SyncManager::new(
        db.clone(),
        transfer_manager.clone(),
        supervisor.clone(),
        budget,
        journal,
        registry.providers_map(),
    );

    SyncRuntimeFixture {
        manager,
        transfer_manager,
        db,
        temp,
        storage_root,
        provider,
        shutdown,
        tracker,
        supervisor,
    }
}

async fn insert_sync_job(
    db: &DbPool,
    id: &str,
    status: &str,
    total: u64,
    synced: u64,
    conflicts: u64,
) {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO sync_jobs (id, user_id, source_connection_id, source_path, destination_connection_id, destination_path, status, strategy, total_files, synced_files, conflict_files, created_at, updated_at) VALUES (?, 'recovery-user', 'local', '/src', 'local', '/dst', ?, 'source_wins', ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(status)
    .bind(total as i64)
    .bind(synced as i64)
    .bind(conflicts as i64)
    .bind(&now)
    .bind(&now)
    .execute(db)
    .await
    .unwrap();
}

async fn insert_operation(
    db: &DbPool,
    id: &str,
    job_id: &str,
    kind: &str,
    status: &str,
    transfer_job_id: Option<&str>,
) {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO sync_operations (id, job_id, op_kind, relative_path, old_path, status, transfer_job_id, error_message, created_at, updated_at) VALUES (?, ?, ?, ?, NULL, ?, ?, NULL, ?, ?)",
    )
    .bind(id)
    .bind(job_id)
    .bind(kind)
    .bind(format!("file-{id}.txt"))
    .bind(status)
    .bind(transfer_job_id)
    .bind(&now)
    .bind(&now)
    .execute(db)
    .await
    .unwrap();
}

fn completed_transfer(id: &str) -> TransferJob {
    let now = Utc::now();
    TransferJob {
        id: id.into(),
        user_id: Some("recovery-user".into()),
        name: format!("{id}.bin"),
        transfer_type: TransferType::Copy,
        source_connection_id: "local".into(),
        source_path: "/src/source.bin".into(),
        destination_connection_id: "local".into(),
        destination_path: "/dst/source.bin".into(),
        status: TransferStatus::Completed,
        phase: TransferPhase::Completed,
        execution_mode: TransferExecutionMode::Background,
        staging: Default::default(),
        transferred_bytes: 42,
        total_bytes: 42,
        speed_bytes_per_sec: 0,
        eta_seconds: Some(0),
        checksum: Some("completed-checksum".into()),
        error_message: None,
        dismissed_at: None,
        created_at: now,
        updated_at: now,
    }
}

#[tokio::test]
async fn executing_recovery_reconciles_multiple_operation_pages_to_completion() {
    let fixture = setup_fixture().await;
    std::fs::create_dir_all(fixture.storage_root.join("src")).unwrap();
    std::fs::create_dir_all(fixture.storage_root.join("dst")).unwrap();
    insert_sync_job(&fixture.db, "sync-many-ops", "executing", 300, 0, 0).await;
    for index in 0..300usize {
        insert_operation(
            &fixture.db,
            &format!("op-{index:04}"),
            "sync-many-ops",
            "noop",
            "pending",
            None,
        )
        .await;
    }

    fixture.manager.recover_interrupted_jobs().await.unwrap();

    let page = fixture.manager.list_jobs_page(None, Some(10)).await.unwrap();
    let job = page
        .items
        .into_iter()
        .find(|job| job.id == "sync-many-ops")
        .expect("recovered sync must remain queryable from durable history");
    assert_eq!(job.status, SyncStatus::Completed);
    assert_eq!(job.total_files, 300);
    assert_eq!(job.synced_files, 300);
    assert_eq!(job.conflict_files, 0);

    let operations = fixture
        .manager
        .list_operations_page("sync-many-ops", None, Some(500))
        .await
        .unwrap();
    assert_eq!(operations.items.len(), 300);
    assert!(operations.items.iter().all(|op| op.status == "completed"));
}

#[tokio::test]
async fn completed_transfer_projection_is_idempotent_during_recovery() {
    let fixture = setup_fixture().await;
    std::fs::create_dir_all(fixture.storage_root.join("src")).unwrap();
    std::fs::create_dir_all(fixture.storage_root.join("dst")).unwrap();
    let transfer_id = "completed-transfer-1";
    fixture
        .transfer_manager
        .insert_job_for_test(completed_transfer(transfer_id))
        .await;
    insert_sync_job(&fixture.db, "sync-transfer-map", "executing", 1, 0, 0).await;
    insert_operation(
        &fixture.db,
        "mapped-op",
        "sync-transfer-map",
        "create",
        "running",
        Some(transfer_id),
    )
    .await;

    fixture.manager.recover_interrupted_jobs().await.unwrap();
    fixture.manager.recover_interrupted_jobs().await.unwrap();

    let row: (String, i64, i64) = sqlx::query_as(
        "SELECT status, synced_files, conflict_files FROM sync_jobs WHERE id = 'sync-transfer-map'",
    )
    .fetch_one(&fixture.db)
    .await
    .unwrap();
    assert_eq!(row.0, "completed");
    assert_eq!(row.1, 1, "completed transfer projection must increment exactly once");
    assert_eq!(row.2, 0);

    let operation: (String,) =
        sqlx::query_as("SELECT status FROM sync_operations WHERE id = 'mapped-op'")
            .fetch_one(&fixture.db)
            .await
            .unwrap();
    assert_eq!(operation.0, "completed");
}

#[tokio::test]
async fn scanning_job_restart_converges_again_without_scheduler_sleep_assumptions() {
    let fixture = setup_fixture().await;
    std::fs::create_dir_all(fixture.storage_root.join("src")).unwrap();
    std::fs::create_dir_all(fixture.storage_root.join("dst")).unwrap();
    insert_sync_job(&fixture.db, "sync-restart", "scanning", 0, 0, 0).await;

    fixture.manager.recover_interrupted_jobs().await.unwrap();

    let job = eventually_default("restarted scanning sync to become terminal", || async {
        let page = fixture.manager.list_jobs_page(None, Some(10)).await.ok()?;
        page.items.into_iter().find(|job| {
            job.id == "sync-restart"
                && matches!(job.status, SyncStatus::Completed | SyncStatus::Failed)
        })
    })
    .await;
    assert_eq!(job.status, SyncStatus::Completed);
    assert_eq!(job.total_files, 0);

    assert!(fixture.supervisor.shutdown(Duration::from_secs(2)).await);
}

#[tokio::test]
async fn streaming_plan_emits_bounded_batches_for_large_manifest() {
    let fixture = setup_fixture().await;
    let src = fixture.storage_root.join("src");
    let dst = fixture.storage_root.join("dst");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::create_dir_all(&dst).unwrap();
    for index in 0..300usize {
        std::fs::write(src.join(format!("file-{index:04}.txt")), b"x").unwrap();
    }

    let cancel = CancellationToken::new();
    let mut plan = StreamingSyncPlan::prepare(
        &fixture.db,
        Arc::clone(&fixture.provider),
        "local",
        "/src",
        Arc::clone(&fixture.provider),
        "local",
        "/dst",
        SyncStrategy::SourceWins,
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!(plan.total_operations(), 300);

    let mut total = 0usize;
    let mut batches = 0usize;
    while let Some(batch) = plan.next_batch().await.unwrap() {
        assert!(!batch.is_empty());
        assert!(batch.len() <= SCAN_BATCH_SIZE);
        total += batch.len();
        batches += 1;
    }
    assert_eq!(total, 300);
    assert_eq!(batches, 2);
}
