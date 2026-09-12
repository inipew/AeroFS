use crate::domain::{Actor, ConnectionId, VfsPath};
use crate::errors::AppError;
use crate::ports::{
    authorization::{Authorization, FileAction},
    effects::FileMutationEffects,
    filesystem::FileSystemResolver,
};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ChmodEntryCommand {
    pub connection: ConnectionId,
    pub path: String,
    pub mode: u32,
}

#[derive(Clone)]
pub struct ChmodEntry {
    authorization: Arc<dyn Authorization>,
    filesystem: Arc<dyn FileSystemResolver>,
    effects: Arc<dyn FileMutationEffects>,
}

impl ChmodEntry {
    pub fn new(
        authorization: Arc<dyn Authorization>,
        filesystem: Arc<dyn FileSystemResolver>,
        effects: Arc<dyn FileMutationEffects>,
    ) -> Self {
        Self {
            authorization,
            filesystem,
            effects,
        }
    }

    pub async fn execute(&self, actor: &Actor, command: ChmodEntryCommand) -> Result<(), AppError> {
        self.authorization
            .authorize(actor, &command.connection, FileAction::Write)
            .await?;
        let provider = self.filesystem.resolve(&command.connection).await?;
        let path = VfsPath::new(command.connection.as_str(), command.path)?;
        provider
            .set_permissions(&path, &format!("{:04o}", command.mode))
            .await?;
        self.effects
            .invalidate(&command.connection, &path.path)
            .await;
        self.effects
            .file_changed(
                actor,
                &command.connection,
                &path.path,
                "FILE_CHMOD",
                "chmod",
                Some(format!("Mode changed to: {:04o}", command.mode)),
            )
            .await;
        Ok(())
    }
}
