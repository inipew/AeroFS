use crate::domain::{Actor, ConnectionId, VfsPath};
use crate::errors::AppError;
use crate::ports::{
    authorization::{Authorization, FileAction},
    filesystem::FileSystemResolver,
    transfer::{TransferEffects, TransferQueue, TransferSubmission},
};
use crate::transfer::TransferType;
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

        let source = VfsPath::new(command.source_connection.as_str(), command.source_path.clone())?;
        let destination = VfsPath::new(
            command.destination_connection.as_str(),
            command.destination_path.clone(),
        )?;
        if source.connection_id == destination.connection_id && source.path == destination.path {
            return Err(AppError::BadRequest(
                "Source and destination must be different".into(),
            ));
        }

        // Resolve both sides before admission so queued work cannot start with an
        // already-missing provider. The engine may still re-resolve on execution/retry.
        self.filesystem.resolve(&command.source_connection).await?;
        self.filesystem.resolve(&command.destination_connection).await?;

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
}
