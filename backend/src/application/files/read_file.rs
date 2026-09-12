use crate::domain::{Actor, ConnectionId, FileKind, FileMetadata, VfsPath};
use crate::errors::AppError;
use crate::ports::{
    authorization::{Authorization, FileAction},
    effects::FileAccessEffects,
    filesystem::FileSystemResolver,
};
use crate::vfs::FileSystem;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ReadFileCommand {
    pub connection: ConnectionId,
    pub path: String,
    pub download: bool,
}

pub struct ReadFileResult {
    pub filesystem: Arc<dyn FileSystem>,
    pub path: VfsPath,
    pub metadata: FileMetadata,
}

#[derive(Clone)]
pub struct ReadFile {
    authorization: Arc<dyn Authorization>,
    filesystem: Arc<dyn FileSystemResolver>,
    access_effects: Arc<dyn FileAccessEffects>,
}

impl ReadFile {
    pub fn new(
        authorization: Arc<dyn Authorization>,
        filesystem: Arc<dyn FileSystemResolver>,
        access_effects: Arc<dyn FileAccessEffects>,
    ) -> Self {
        Self {
            authorization,
            filesystem,
            access_effects,
        }
    }

    pub async fn execute(
        &self,
        actor: &Actor,
        command: ReadFileCommand,
    ) -> Result<ReadFileResult, AppError> {
        let action = if command.download {
            FileAction::Download
        } else {
            FileAction::Read
        };
        self.authorization
            .authorize(actor, &command.connection, action)
            .await?;

        let path = VfsPath::new(command.connection.as_str(), command.path)?;
        let filesystem = self.filesystem.resolve(&command.connection).await?;
        let metadata = filesystem.stat(&path).await?;
        if metadata.kind != FileKind::File {
            return Err(AppError::BadRequest("Target is not a regular file".into()));
        }

        if command.download {
            self.access_effects
                .accessed(
                    actor,
                    &command.connection,
                    &path.path,
                    "FILE_DOWNLOAD",
                    Some(format!("Downloaded: {}", path.path)),
                )
                .await;
        }

        Ok(ReadFileResult {
            filesystem,
            path,
            metadata,
        })
    }
}
