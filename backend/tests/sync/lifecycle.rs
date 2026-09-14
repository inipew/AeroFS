use crate::support::{
    create_local_sync_job, list_sync_operations_page, transfer_admin_actor, wait_sync_status,
    wait_sync_terminal, TestAppBuilder,
};
use axum::extract::FromRef;
use backend::{
    state::SyncState,
    sync::{SyncStatus, SyncStrategy},
};

#[tokio::test]
async fn source_wins_sync_copies_new_file_and_persists_terminal_history() {
    let payload = b"sync payload from source".to_vec();
    let app = TestAppBuilder::new()
        .running()
        .with_file("src/new.txt", payload.clone())
        .build()
        .await;
    std::fs::create_dir_all(app.storage_path("dst")).unwrap();

    let admin = transfer_admin_actor();
    let created = create_local_sync_job(
        &app,
        &admin,
        "/src",
        "/dst",
        SyncStrategy::SourceWins,
    )
    .await;

    let terminal = wait_sync_terminal(&app, &created.id).await;
    assert_eq!(terminal.status, SyncStatus::Completed);
    assert_eq!(terminal.total_files, 1);
    assert_eq!(terminal.synced_files, 1);
    assert_eq!(terminal.conflict_files, 0);
    assert_eq!(std::fs::read(app.storage_path("dst/new.txt")).unwrap(), payload);

    let operations = list_sync_operations_page(&app, &created.id, None, Some(10)).await;
    assert_eq!(operations.items.len(), 1);
    assert_eq!(operations.items[0].op_kind, "create");
    assert_eq!(operations.items[0].status, "completed");

    let durable: (String, i64, i64, i64) = sqlx::query_as(
        "SELECT status, total_files, synced_files, conflict_files FROM sync_jobs WHERE id = ?",
    )
    .bind(&created.id)
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert_eq!(durable.0, "completed");
    assert_eq!((durable.1, durable.2, durable.3), (1, 1, 0));
}

#[tokio::test]
async fn keep_both_conflict_is_durable_and_resolution_updates_the_operation() {
    let app = TestAppBuilder::new()
        .running()
        .with_file("src/shared.txt", b"source version".to_vec())
        .with_file(
            "dst/shared.txt",
            b"destination version with a deliberately different size".to_vec(),
        )
        .build()
        .await;

    let admin = transfer_admin_actor();
    let created = create_local_sync_job(
        &app,
        &admin,
        "/src",
        "/dst",
        SyncStrategy::KeepBoth,
    )
    .await;

    let conflicted = wait_sync_status(&app, &created.id, &[SyncStatus::Conflict]).await;
    assert_eq!(conflicted.total_files, 1);
    assert_eq!(conflicted.synced_files, 0);
    assert_eq!(conflicted.conflict_files, 1);

    let operations = list_sync_operations_page(&app, &created.id, None, Some(10)).await;
    assert_eq!(operations.items.len(), 1);
    let op = &operations.items[0];
    assert_eq!(op.op_kind, "conflict");
    assert_eq!(op.status, "conflict");

    let sync = SyncState::from_ref(&app.state);
    sync.service
        .resolve_conflict(&created.id, &op.id, "use_dest")
        .await
        .expect("use_dest conflict resolution should succeed");

    let resolved = list_sync_operations_page(&app, &created.id, None, Some(10)).await;
    assert_eq!(resolved.items.len(), 1);
    assert_eq!(resolved.items[0].status, "completed");
    assert_eq!(
        std::fs::read(app.storage_path("dst/shared.txt")).unwrap(),
        b"destination version with a deliberately different size"
    );
}
