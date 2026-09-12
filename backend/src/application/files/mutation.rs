use crate::domain::{Actor, ConnectionId, FileMetadata, PermissionInheritanceMode, VfsPath};
use crate::errors::AppError;
use crate::ports::{
    authorization::{Authorization, FileAction},
    effects::FileMutationEffects,
    filesystem::FileSystemResolver,
};
use futures::{stream, StreamExt};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct CreateDirectoryCommand {
    pub connection: ConnectionId,
    pub path: String,
}

#[derive(Clone)]
pub struct CreateDirectory {
    authorization: Arc<dyn Authorization>,
    filesystem: Arc<dyn FileSystemResolver>,
    effects: Arc<dyn FileMutationEffects>,
}

impl CreateDirectory {
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

    pub async fn execute(
        &self,
        actor: &Actor,
        command: CreateDirectoryCommand,
    ) -> Result<FileMetadata, AppError> {
        use crate::domain::policy::resolve_destination_permissions;

        self.authorization
            .authorize(actor, &command.connection, FileAction::Create)
            .await?;
        let provider = self.filesystem.resolve(&command.connection).await?;
        let path = VfsPath::new(command.connection.as_str(), command.path.clone())?;
        let permissions = resolve_destination_permissions(
            &provider,
            &path,
            true,
            PermissionInheritanceMode::InheritParent,
        )
        .await;

        provider.create_dir(&path).await?;
        if let Some(permissions) = permissions {
            let _ = provider.set_permissions(&path, &permissions).await;
        }
        let metadata = provider.stat(&path).await?;

        self.effects
            .invalidate(&command.connection, &command.path)
            .await;
        self.effects
            .file_changed(
                actor,
                &command.connection,
                &path.path,
                "FILE_MKDIR",
                "create",
                None,
            )
            .await?;
        Ok(metadata)
    }
}

#[derive(Debug, Clone)]
pub struct RenameEntryCommand {
    pub connection: ConnectionId,
    pub from: String,
    pub to: String,
}

#[derive(Clone)]
pub struct RenameEntry {
    authorization: Arc<dyn Authorization>,
    filesystem: Arc<dyn FileSystemResolver>,
    effects: Arc<dyn FileMutationEffects>,
}

impl RenameEntry {
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

    pub async fn execute(
        &self,
        actor: &Actor,
        command: RenameEntryCommand,
    ) -> Result<(), AppError> {
        self.authorization
            .authorize(actor, &command.connection, FileAction::Write)
            .await?;
        self.authorization
            .authorize(actor, &command.connection, FileAction::Delete)
            .await?;

        let provider = self.filesystem.resolve(&command.connection).await?;
        let from = VfsPath::new(command.connection.as_str(), command.from.clone())?;
        let to = VfsPath::new(command.connection.as_str(), command.to.clone())?;
        provider.rename(&from, &to).await?;

        self.effects
            .invalidate_prefix(&command.connection, &command.from)
            .await;
        self.effects
            .invalidate_prefix(&command.connection, &command.to)
            .await;
        self.effects
            .file_renamed(actor, &command.connection, &from.path, &to.path)
            .await?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CopyEntryCommand {
    pub connection: ConnectionId,
    pub from: String,
    pub to: String,
}

#[derive(Clone)]
pub struct CopyEntry {
    authorization: Arc<dyn Authorization>,
    filesystem: Arc<dyn FileSystemResolver>,
    effects: Arc<dyn FileMutationEffects>,
}

impl CopyEntry {
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

    pub async fn execute(
        &self,
        actor: &Actor,
        command: CopyEntryCommand,
    ) -> Result<String, AppError> {
        self.authorization
            .authorize(actor, &command.connection, FileAction::Read)
            .await?;
        self.authorization
            .authorize(actor, &command.connection, FileAction::Create)
            .await?;

        let provider = self.filesystem.resolve(&command.connection).await?;
        let from = VfsPath::new(command.connection.as_str(), command.from)?;
        let to = VfsPath::new(command.connection.as_str(), command.to)?;
        if from.path == to.path {
            return Err(AppError::BadRequest(
                "Source and destination must be different".into(),
            ));
        }

        provider.copy(&from, &to).await?;
        self.effects
            .invalidate_prefix(&command.connection, &to.path)
            .await;
        self.effects
            .file_copied(actor, &command.connection, &from.path, &to.path)
            .await?;
        Ok(to.path)
    }
}

#[derive(Debug, Clone)]
pub struct DeleteEntriesCommand {
    pub connection: ConnectionId,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct DeleteEntriesResult {
    pub succeeded: Vec<String>,
    pub failed: Vec<(String, String)>,
}

#[derive(Clone)]
pub struct DeleteEntries {
    authorization: Arc<dyn Authorization>,
    filesystem: Arc<dyn FileSystemResolver>,
    effects: Arc<dyn FileMutationEffects>,
}

impl DeleteEntries {
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

    pub async fn execute(
        &self,
        actor: &Actor,
        command: DeleteEntriesCommand,
    ) -> Result<DeleteEntriesResult, AppError> {
        self.authorization
            .authorize(actor, &command.connection, FileAction::Delete)
            .await?;
        let provider = self.filesystem.resolve(&command.connection).await?;
        let connection = command.connection.clone();

        let mut deletes = stream::iter(command.paths.into_iter().map(|raw_path| {
            let provider = provider.clone();
            let connection = connection.clone();
            async move {
                let path = match VfsPath::new(connection.as_str(), &raw_path) {
                    Ok(path) => path,
                    Err(error) => return (raw_path, Err(error)),
                };
                let result = provider.delete(&path).await;
                (raw_path, result)
            }
        }))
        .buffer_unordered(8);

        let mut succeeded = Vec::new();
        let mut failed = Vec::new();
        while let Some((path, result)) = deletes.next().await {
            match result {
                Ok(()) => {
                    self.effects.invalidate_prefix(&connection, &path).await;
                    match self
                        .effects
                        .file_changed(
                            actor,
                            &connection,
                            &path,
                            "FILE_DELETE",
                            "delete",
                            None,
                        )
                        .await
                    {
                        Ok(()) => succeeded.push(path),
                        Err(error) => failed.push((path, error.to_string())),
                    }
                }
                Err(error) => failed.push((path, error.to_string())),
            }
        }

        succeeded.sort();
        failed.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(DeleteEntriesResult { succeeded, failed })
    }
}
