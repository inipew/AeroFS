use super::{eventually_default, TestApp};
use axum::extract::FromRef;
use backend::{
    domain::Actor,
    state::SyncState,
    sync::{SyncHistoryPage, SyncJob, SyncOperationRow, SyncPageCursor, SyncStatus, SyncStrategy},
};

pub async fn create_sync_job(
    app: &TestApp,
    actor: &Actor,
    source_connection: &str,
    source_path: &str,
    destination_connection: &str,
    destination_path: &str,
    strategy: SyncStrategy,
) -> SyncJob {
    let sync = SyncState::from_ref(&app.state);
    sync.service
        .create_job(
            actor,
            source_connection,
            source_path,
            destination_connection,
            destination_path,
            strategy,
        )
        .await
        .expect("sync submission should succeed")
}

pub async fn create_local_sync_job(
    app: &TestApp,
    actor: &Actor,
    source_path: &str,
    destination_path: &str,
    strategy: SyncStrategy,
) -> SyncJob {
    create_sync_job(
        app,
        actor,
        "local",
        source_path,
        "local",
        destination_path,
        strategy,
    )
    .await
}

pub async fn list_sync_jobs_page(
    app: &TestApp,
    cursor: Option<&SyncPageCursor>,
    limit: Option<usize>,
) -> SyncHistoryPage<SyncJob> {
    let sync = SyncState::from_ref(&app.state);
    sync.service
        .list_jobs(cursor, limit)
        .await
        .expect("list sync jobs")
}

pub async fn list_sync_operations_page(
    app: &TestApp,
    job_id: &str,
    cursor: Option<&SyncPageCursor>,
    limit: Option<usize>,
) -> SyncHistoryPage<SyncOperationRow> {
    let sync = SyncState::from_ref(&app.state);
    sync.service
        .list_operations(job_id, cursor, limit)
        .await
        .expect("list sync operations")
}

pub async fn find_sync_job(app: &TestApp, job_id: &str) -> Option<SyncJob> {
    list_sync_jobs_page(app, None, Some(500))
        .await
        .items
        .into_iter()
        .find(|job| job.id == job_id)
}

pub async fn wait_sync_status(
    app: &TestApp,
    job_id: &str,
    accepted: &[SyncStatus],
) -> SyncJob {
    let description = format!("sync {job_id} to reach one of {accepted:?}");
    eventually_default(&description, || async {
        let job = find_sync_job(app, job_id).await?;
        accepted.contains(&job.status).then_some(job)
    })
    .await
}

pub async fn wait_sync_terminal(app: &TestApp, job_id: &str) -> SyncJob {
    wait_sync_status(
        app,
        job_id,
        &[SyncStatus::Completed, SyncStatus::Failed, SyncStatus::Conflict],
    )
    .await
}
