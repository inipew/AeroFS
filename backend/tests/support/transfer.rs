use super::{eventually_default, TestApp};
use axum::extract::FromRef;
use backend::{
    application::transfers::CreateTransferCommand,
    domain::{Actor, ConnectionId},
    ports::transfer::{TransferJobResponse, TransferStatus, TransferType},
    state::TransferState,
};

pub fn actor(id: impl Into<String>, username: impl Into<String>, is_admin: bool) -> Actor {
    Actor {
        id: id.into(),
        username: username.into(),
        is_admin,
    }
}

pub fn admin_actor() -> Actor {
    actor("test-admin", "admin", true)
}

pub async fn create_transfer(
    app: &TestApp,
    actor: &Actor,
    name: impl Into<String>,
    transfer_type: TransferType,
    source_connection: &str,
    source_path: &str,
    destination_connection: &str,
    destination_path: &str,
) -> String {
    let transfers = TransferState::from_ref(&app.state);
    transfers
        .use_cases
        .create_transfer
        .execute(
            actor,
            CreateTransferCommand {
                name: name.into(),
                transfer_type,
                source_connection: ConnectionId::new(source_connection)
                    .expect("valid source connection id"),
                source_path: source_path.to_string(),
                destination_connection: ConnectionId::new(destination_connection)
                    .expect("valid destination connection id"),
                destination_path: destination_path.to_string(),
            },
        )
        .await
        .expect("transfer submission should succeed")
}

pub async fn create_local_transfer(
    app: &TestApp,
    actor: &Actor,
    name: impl Into<String>,
    transfer_type: TransferType,
    source_path: &str,
    destination_path: &str,
) -> String {
    create_transfer(
        app,
        actor,
        name,
        transfer_type,
        "local",
        source_path,
        "local",
        destination_path,
    )
    .await
}

pub async fn list_transfers(app: &TestApp, actor: &Actor) -> Vec<TransferJobResponse> {
    let transfers = TransferState::from_ref(&app.state);
    transfers
        .use_cases
        .list(actor)
        .await
        .expect("list transfers")
}

pub async fn find_transfer(
    app: &TestApp,
    actor: &Actor,
    job_id: &str,
) -> Option<TransferJobResponse> {
    list_transfers(app, actor)
        .await
        .into_iter()
        .find(|job| job.id == job_id)
}

pub async fn wait_for_status(
    app: &TestApp,
    actor: &Actor,
    job_id: &str,
    accepted: &[TransferStatus],
) -> TransferJobResponse {
    let description = format!("transfer {job_id} to reach one of {accepted:?}");
    eventually_default(&description, || async {
        let job = find_transfer(app, actor, job_id).await?;
        accepted.contains(&job.status).then_some(job)
    })
    .await
}

pub async fn wait_completed(app: &TestApp, actor: &Actor, job_id: &str) -> TransferJobResponse {
    wait_for_status(app, actor, job_id, &[TransferStatus::Completed]).await
}

pub async fn wait_failed(app: &TestApp, actor: &Actor, job_id: &str) -> TransferJobResponse {
    wait_for_status(app, actor, job_id, &[TransferStatus::Failed]).await
}

pub async fn wait_terminal(app: &TestApp, actor: &Actor, job_id: &str) -> TransferJobResponse {
    wait_for_status(
        app,
        actor,
        job_id,
        &[
            TransferStatus::Completed,
            TransferStatus::Failed,
            TransferStatus::Cancelled,
            TransferStatus::Interrupted,
        ],
    )
    .await
}
