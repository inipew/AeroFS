use crate::domain::{Actor, ConnectionId, FileMetadata, VfsPath};
use crate::errors::AppError;
use crate::ports::{
    authorization::{Authorization, FileAction},
    filesystem::FileSystemResolver,
};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct StatFileCommand {
    pub connection: ConnectionId,
    pub path: String,
}

#[derive(Clone)]
pub struct StatFile {
    authorization: Arc<dyn Authorization>,
    filesystem: Arc<dyn FileSystemResolver>,
}

impl StatFile {
    pub fn new(
        authorization: Arc<dyn Authorization>,
        filesystem: Arc<dyn FileSystemResolver>,
    ) -> Self {
        Self {
            authorization,
            filesystem,
        }
    }
    pub async fn execute(
        &self,
        actor: &Actor,
        command: StatFileCommand,
    ) -> Result<FileMetadata, AppError> {
        self.authorization
            .authorize(actor, &command.connection, FileAction::List)
            .await?;
        let path = VfsPath::new(command.connection.as_str(), command.path)?;
        self.filesystem
            .resolve(&command.connection)
            .await?
            .stat(&path)
            .await
            .map_err(Into::into)
    }
}
