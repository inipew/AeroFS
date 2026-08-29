use backend::events::{DomainEvent, EventJournal, ReplayOutcome};
use backend::runtime::{ResourceBudget, TaskSupervisor};
use backend::state::{AppRuntime, RuntimePhase};
use backend::sync::{ConflictResolver, FileManifest, ManifestDiffer, SyncOpKind, SyncStrategy};
use backend::transfer::TransferCheckpoint;
use backend::vfs::ProviderState;
use chrono::Utc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_task_supervisor_tracked_spawns_and_drain() {
    let supervisor = TaskSupervisor::new();
    assert_eq!(supervisor.active_tasks(), 0);

    let flag = Arc::new(AtomicBool::new(false));
    let flag_clone = Arc::clone(&flag);

    supervisor.spawn("worker_test", async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        flag_clone.store(true, Ordering::SeqCst);
    });

    assert_eq!(supervisor.active_tasks(), 1);

    let drained = supervisor.shutdown(Duration::from_secs(2)).await;
    assert!(drained);
    assert!(flag.load(Ordering::SeqCst));
    assert_eq!(supervisor.active_tasks(), 0);
}

#[tokio::test]
async fn test_runtime_binding_phase_and_health_readiness() {
    let runtime = AppRuntime::default();
    assert_eq!(runtime.phase(), RuntimePhase::Starting);

    runtime.set_phase(RuntimePhase::Binding);
    assert_eq!(runtime.phase(), RuntimePhase::Binding);
    assert_eq!(runtime.phase().as_str(), "binding");
    assert!(!runtime.is_shutting_down());

    runtime.set_phase(RuntimePhase::Running);
    assert_eq!(runtime.phase(), RuntimePhase::Running);
    assert_eq!(runtime.phase().as_str(), "running");
    assert!(!runtime.is_shutting_down());

    runtime.set_phase(RuntimePhase::ShuttingDown);
    assert!(runtime.is_shutting_down());
    assert_eq!(runtime.phase().as_str(), "shutting_down");
}

#[tokio::test]
async fn test_event_journal_epoch_and_sqlite_persistence() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .unwrap();

    let migrator = sqlx::migrate!("./migrations");
    migrator.run(&pool).await.unwrap();

    let journal = EventJournal::init(pool.clone()).await.unwrap();
    let epoch = journal.epoch().to_string();
    assert!(!epoch.is_empty());

    let event1 = DomainEvent::file_change("local", "/docs/test.txt", "create");
    let env1 = journal.append(event1, Some("/docs/test.txt")).await.unwrap();
    assert_eq!(env1.sequence, 1);
    assert_eq!(env1.epoch, epoch);

    let event2 = DomainEvent::file_rename("local", "/docs/test.txt", "/docs/renamed.txt");
    let env2 = journal.append(event2, Some("/docs/renamed.txt")).await.unwrap();
    assert_eq!(env2.sequence, 2);
    assert_eq!(env2.epoch, epoch);

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Query replay with matching epoch
    let outcome = journal.get_since(Some(&epoch), 0, 10).await.unwrap();
    match outcome {
        ReplayOutcome::Events(events) => {
            assert_eq!(events.len(), 2);
            assert_eq!(events[0].sequence, 1);
            assert_eq!(events[1].sequence, 2);
        }
        _ => panic!("Expected ReplayOutcome::Events"),
    }
}

#[tokio::test]
async fn test_event_journal_epoch_mismatch_triggers_full_sync() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .unwrap();

    let migrator = sqlx::migrate!("./migrations");
    migrator.run(&pool).await.unwrap();

    let journal = EventJournal::init(pool).await.unwrap();
    let current_epoch = journal.epoch();

    let outcome = journal.get_since(Some("old-expired-epoch-1234"), 50, 10).await.unwrap();
    match outcome {
        ReplayOutcome::EpochMismatch { current_epoch: reported_epoch, .. } => {
            assert_eq!(reported_epoch, current_epoch);
        }
        _ => panic!("Expected ReplayOutcome::EpochMismatch"),
    }
}

#[tokio::test]
async fn test_transfer_checkpoint_save_load_delete() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .unwrap();

    let migrator = sqlx::migrate!("./migrations");
    migrator.run(&pool).await.unwrap();

    let transfer_id = "test-transfer-ckpt-1";
    let checkpoint = TransferCheckpoint {
        transfer_id: transfer_id.to_string(),
        offset: 1048576,
        total: 5242880,
        staging_path: "/.file.txt.aerofs-part-1".to_string(),
        source_etag: Some("\"etag-abc-123\"".to_string()),
        source_version: None,
        checksum_so_far: None,
        updated_at: Utc::now(),
    };

    checkpoint.save(&pool).await.unwrap();

    let loaded = TransferCheckpoint::load(&pool, transfer_id).await.unwrap().expect("Checkpoint must exist");
    assert_eq!(loaded.transfer_id, transfer_id);
    assert_eq!(loaded.offset, 1048576);
    assert_eq!(loaded.total, 5242880);
    assert_eq!(loaded.source_etag, Some("\"etag-abc-123\"".to_string()));

    TransferCheckpoint::delete(&pool, transfer_id).await.unwrap();
    let after_delete = TransferCheckpoint::load(&pool, transfer_id).await.unwrap();
    assert!(after_delete.is_none());
}

#[tokio::test]
async fn test_provider_lifecycle_states() {
    let s_init = ProviderState::Initializing;
    assert_eq!(s_init.as_str(), "initializing");
    assert!(!s_init.is_ready());
    assert!(!s_init.is_operational());

    let s_ready = ProviderState::Ready;
    assert_eq!(s_ready.as_str(), "ready");
    assert!(s_ready.is_ready());
    assert!(s_ready.is_operational());

    let s_degraded = ProviderState::Degraded {
        since: Utc::now(),
        reason: "Rate limit 429".to_string(),
    };
    assert_eq!(s_degraded.as_str(), "degraded");
    assert!(!s_degraded.is_ready());
    assert!(s_degraded.is_operational());

    let s_draining = ProviderState::Draining;
    assert_eq!(s_draining.as_str(), "draining");
    assert!(!s_draining.is_ready());
    assert!(!s_draining.is_operational());
}

#[tokio::test]
async fn test_resource_budget_concurrency_coordination() {
    let budget = ResourceBudget::new(4, 2, 2, 1, 1);

    let p1 = budget.acquire_local_disk().await.expect("Must acquire permit");
    let p2 = budget.acquire_local_disk().await.expect("Must acquire permit");
    assert_eq!(budget.local_disk.available_permits(), 0);

    drop(p1);
    assert_eq!(budget.local_disk.available_permits(), 1);
    drop(p2);
    assert_eq!(budget.local_disk.available_permits(), 2);
}

#[tokio::test]
async fn test_sync_manifest_diff_engine() {
    let src = vec![
        FileManifest {
            path: "same.txt".to_string(),
            kind: backend::domain::FileKind::File,
            size: 100,
            modified_at: None,
            content_hash: Some("hash1".to_string()),
            etag: None,
        },
        FileManifest {
            path: "modified.txt".to_string(),
            kind: backend::domain::FileKind::File,
            size: 200,
            modified_at: None,
            content_hash: Some("hash2_src".to_string()),
            etag: None,
        },
        FileManifest {
            path: "new_file.txt".to_string(),
            kind: backend::domain::FileKind::File,
            size: 300,
            modified_at: None,
            content_hash: Some("hash3".to_string()),
            etag: None,
        },
    ];

    let dst = vec![
        FileManifest {
            path: "same.txt".to_string(),
            kind: backend::domain::FileKind::File,
            size: 100,
            modified_at: None,
            content_hash: Some("hash1".to_string()),
            etag: None,
        },
        FileManifest {
            path: "modified.txt".to_string(),
            kind: backend::domain::FileKind::File,
            size: 250,
            modified_at: None,
            content_hash: Some("hash2_dst".to_string()),
            etag: None,
        },
    ];

    // SourceWins strategy:
    let ops_src_wins = ManifestDiffer::diff(&src, &dst, SyncStrategy::SourceWins);
    let same_op = ops_src_wins.iter().find(|o| o.relative_path == "same.txt").unwrap();
    assert_eq!(same_op.kind, SyncOpKind::Noop);

    let mod_op = ops_src_wins.iter().find(|o| o.relative_path == "modified.txt").unwrap();
    assert_eq!(mod_op.kind, SyncOpKind::Update);

    let new_op = ops_src_wins.iter().find(|o| o.relative_path == "new_file.txt").unwrap();
    assert_eq!(new_op.kind, SyncOpKind::Create);

    // KeepBoth strategy:
    let ops_keep_both = ManifestDiffer::diff(&src, &dst, SyncStrategy::KeepBoth);
    let mod_conflict = ops_keep_both.iter().find(|o| o.relative_path == "modified.txt").unwrap();
    assert_eq!(mod_conflict.kind, SyncOpKind::Conflict);
}

#[test]
fn test_sync_conflict_resolution_filename() {
    let name = "report.pdf";
    let conflict_name = ConflictResolver::generate_conflict_filename(name);
    assert!(conflict_name.starts_with("report (conflict-"));
    assert!(conflict_name.ends_with(").pdf"));

    let no_ext = "README";
    let conflict_no_ext = ConflictResolver::generate_conflict_filename(no_ext);
    assert!(conflict_no_ext.starts_with("README (conflict-"));
    assert!(conflict_no_ext.ends_with(")"));
}
