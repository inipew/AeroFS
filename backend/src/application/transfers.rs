use crate::domain::{Actor, ConnectionId, VfsPath};
use crate::errors::AppError;
use crate::ports::{
    authorization::{Authorization, FileAction},
    filesystem::FileSystemResolver,
    transfer::{TransferControl, TransferEffects, TransferQueue, TransferSubmission},
};
use crate::transfer::{TransferJobResponse, TransferType};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct CreateTransferCommand {
    pub name: String,
    pub transfer_type: TransferType,
    pub source_connection: ConnectionId,
    pub source_path: String,
    pub destination_connection: ConnectionId,
    pub destination_path: String,
}

#[derive(Clone)]
pub struct CreateTransfer {
    authorization: Arc<dyn Authorization>,
    filesystem: Arc<dyn FileSystemResolver>,
    queue: Arc<dyn TransferQueue>,
    effects: Arc<dyn TransferEffects>,
}

impl CreateTransfer {
    pub fn new(
        authorization: Arc<dyn Authorization>,
        filesystem: Arc<dyn FileSystemResolver>,
        queue: Arc<dyn TransferQueue>,
        effects: Arc<dyn TransferEffects>,
    ) -> Self {
        Self {
            authorization,
            filesystem,
            queue,
            effects,
        }
    }

    pub async fn execute(
        &self,
        actor: &Actor,
        command: CreateTransferCommand,
    ) -> Result<String, AppError> {
        self.authorization
            .authorize(actor, &command.source_connection, FileAction::Read)
            .await?;
        if command.transfer_type == TransferType::Move {
            self.authorization
                .authorize(actor, &command.source_connection, FileAction::Delete)
                .await?;
        }
        self.authorization
            .authorize(actor, &command.destination_connection, FileAction::Write)
            .await?;
        self.authorization
            .authorize(actor, &command.destination_connection, FileAction::Create)
            .await?;

        let source = VfsPath::new(
            command.source_connection.as_str(),
            command.source_path.clone(),
        )?;
        let destination = VfsPath::new(
            command.destination_connection.as_str(),
            command.destination_path.clone(),
        )?;
        if source.connection_id == destination.connection_id && source.path == destination.path {
            return Err(AppError::BadRequest(
                "Source and destination must be different".into(),
            ));
        }

        self.filesystem.resolve(&command.source_connection).await?;
        self.filesystem
            .resolve(&command.destination_connection)
            .await?;

        let submission = TransferSubmission {
            user_id: Some(actor.id.clone()),
            name: command.name,
            transfer_type: command.transfer_type,
            source_connection: command.source_connection,
            source_path: source.path,
            destination_connection: command.destination_connection,
            destination_path: destination.path,
        };
        let job_id = self.queue.submit(submission.clone()).await?;
        self.effects.submitted(actor, &submission, &job_id).await;
        Ok(job_id)
    }
}

#[derive(Clone)]
pub struct TransferUseCases {
    pub create_transfer: CreateTransfer,
    control: Arc<dyn TransferControl>,
}

impl TransferUseCases {
    pub fn new(create_transfer: CreateTransfer, control: Arc<dyn TransferControl>) -> Self {
        Self {
            create_transfer,
            control,
        }
    }

    pub async fn list(&self, actor: &Actor) -> Result<Vec<TransferJobResponse>, AppError> {
        self.control.list(actor).await
    }

    pub async fn cancel(&self, actor: &Actor, job_id: &str) -> Result<(), AppError> {
        self.control.cancel(actor, job_id).await
    }

    pub async fn retry(&self, actor: &Actor, job_id: &str) -> Result<(), AppError> {
        self.control.retry(actor, job_id).await
    }

    pub async fn dismiss(&self, actor: &Actor, job_id: &str) -> Result<(), AppError> {
        self.control.dismiss(actor, job_id).await
    }

    pub async fn clear_finished(&self, actor: &Actor) -> Result<usize, AppError> {
        self.control.clear_finished(actor).await
    }
}
