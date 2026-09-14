// Plan 59 lifecycle/architecture regression tests.
//
// These tests intentionally exercise public construction and lifecycle APIs so architectural
// changes cannot silently drift away from the application bootstrap wiring.

use backend::runtime::{ResourceBudget, TaskSupervisor};
use backend::sync::SyncStrategy;
use std::sync::Arc;

#[tokio::test]
async fn sync_manager_can_create_job_with_global_resource_budget() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .unwrap();

    let migrator = sqlx::migrate!("./migrations");
    migrator.run(&pool).await.unwrap();

    let registry = Arc::new(backend::vfs::ProviderRegistry::new());
    let token = tokio_util::sync::CancellationToken::new();
    let tracker = tokio_util::task::TaskTracker::new();

    let event_journal = Arc::new(
        backend::events::EventJournal::init(pool.clone())
            .await
            .unwrap(),
    );
    let resource_budget = Arc::new(ResourceBudget::default());

    let transfer_manager = backend::transfer::TransferManager::new(
        registry.providers_map(),
        pool.clone(),
        4,
        resource_budget.clone(),
        event_journal.clone(),
        token,
        &tracker,
    )
    .await;

    let supervisor = TaskSupervisor::new();
    let sync_mgr = backend::sync::SyncManager::new(
        pool.clone(),
        transfer_manager,
        supervisor,
        resource_budget,
        event_journal,
        registry.providers_map(),
    );

    let job = sync_mgr
        .create_job(
            "user-1",
            "local",
            "/src",
            "local",
            "/dst",
            SyncStrategy::SourceWins,
        )
        .await
        .unwrap();

    assert_eq!(job.user_id, "user-1");
    assert_eq!(job.source_connection_id, "local");
    assert_eq!(job.destination_connection_id, "local");
}
