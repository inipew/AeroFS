use super::FileApplicationService;
use crate::auth::UserInfo;
use crate::domain::{Actor, ConnectionId, FileMetadata, PermissionInheritanceMode, VfsPath};
use crate::errors::{AppError, VfsError};
use crate::ports::{
    authorization::{Authorization, FileAction},
    effects::FileMutationEffects,
    filesystem::FileSystemResolver,
};
use std::sync::Arc;
use tokio::task::JoinSet;

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
            .await;
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
            .await;
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

    pub async fn execute(&self, actor: &Actor, command: CopyEntryCommand) -> Result<String, AppError> {
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
            .file_changed(
                actor,
                &command.connection,
                &to.path,
                "FILE_COPY",
                "copy",
                Some(format!("Copied {} -> {}", from.path, to.path)),
            )
            .await;
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
        let semaphore = Arc::new(tokio::sync::Semaphore::new(8));
        let mut tasks = JoinSet::new();

        for raw_path in command.paths {
            let provider = provider.clone();
            let connection = command.connection.clone();
            let semaphore = semaphore.clone();
            tasks.spawn(async move {
                let _permit = semaphore
                    .acquire()
                    .await
                    .map_err(|_| VfsError::IoError("Semaphore closed".into()))?;
                let path = VfsPath::new(connection.as_str(), &raw_path)?;
                let result = provider.delete(&path).await;
                Ok::<_, VfsError>((raw_path, result))
            });
        }

        let mut succeeded = Vec::new();
        let mut failed = Vec::new();
        while let Some(joined) = tasks.join_next().await {
            match joined {
                Ok(Ok((path, Ok(())))) => {
                    self.effects
                        .invalidate_prefix(&command.connection, &path)
                        .await;
                    self.effects
                        .file_changed(
                            actor,
                            &command.connection,
                            &path,
                            "FILE_DELETE",
                            "delete",
                            None,
                        )
                        .await;
                    succeeded.push(path);
                }
                Ok(Ok((path, Err(error)))) => failed.push((path, error.to_string())),
                Ok(Err(error)) => failed.push(("<invalid-path>".into(), error.to_string())),
                Err(error) => failed.push(("<task>".into(), error.to_string())),
            }
        }

        succeeded.sort();
        failed.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(DeleteEntriesResult { succeeded, failed })
    }
}

fn actor_from_user(user: &UserInfo) -> Actor {
    Actor {
        id: user.id.clone(),
        username: user.username.clone(),
        is_admin: user.is_admin,
    }
}

impl FileApplicationService {
    pub async fn create_directory_typed(
        &self,
        user: &UserInfo,
        connection: &ConnectionId,
        raw_path: String,
    ) -> Result<FileMetadata, AppError> {
        CreateDirectory::new(
            self.authorization.clone(),
            self.filesystem.clone(),
            self.effects.clone(),
        )
        .execute(
            &actor_from_user(user),
            CreateDirectoryCommand {
                connection: connection.clone(),
                path: raw_path,
            },
        )
        .await
    }

    pub async fn delete_files_typed(
        &self,
        user: &UserInfo,
        connection: &ConnectionId,
        paths: Vec<String>,
    ) -> Result<(Vec<String>, Vec<(String, String)>), AppError> {
        let result = DeleteEntries::new(
            self.authorization.clone(),
            self.filesystem.clone(),
            self.effects.clone(),
        )
        .execute(
            &actor_from_user(user),
            DeleteEntriesCommand {
                connection: connection.clone(),
                paths,
            },
        )
        .await?;
        Ok((result.succeeded, result.failed))
    }

    pub async fn rename_typed(
        &self,
        user: &UserInfo,
        connection: &ConnectionId,
        from_raw: String,
        to_raw: String,
    ) -> Result<(), AppError> {
        RenameEntry::new(
            self.authorization.clone(),
            self.filesystem.clone(),
            self.effects.clone(),
        )
        .execute(
            &actor_from_user(user),
            RenameEntryCommand {
                connection: connection.clone(),
                from: from_raw,
                to: to_raw,
            },
        )
        .await
    }

    pub async fn chmod_typed(
        &self,
        user: &UserInfo,
        connection: &ConnectionId,
        raw_path: String,
        mode: u32,
    ) -> Result<(), AppError> {
        use crate::auth::permissions::{check_permission, PermissionAction};
        check_permission(&self.db, user, connection.as_str(), PermissionAction::Write).await?;
        let provider = self
            .registry
            .get(connection.as_str())
            .await
            .ok_or_else(|| {
                VfsError::ConnectionError(format!("Connection '{}' not found", connection.as_str()))
            })?;
        let vfs_path = VfsPath::new(connection.as_str(), raw_path.clone())?;
        let mode_str = format!("{:04o}", mode);
        provider.set_permissions(&vfs_path, &mode_str).await?;
        self.metadata_cache
            .invalidate(connection.as_str(), &raw_path)
            .await;
        crate::auth::audit::record_audit_log(
            &self.db,
            Some(&user.id),
            "FILE_CHMOD",
            Some(connection.as_str()),
            Some(&vfs_path.path),
            "SUCCESS",
            None,
            Some(&format!("Mode changed to: {:04o}", mode)),
        )
        .await;
        let _ = self
            .event_journal
            .append(
                crate::events::DomainEvent::file_change(
                    connection.as_str(),
                    &vfs_path.path,
                    "chmod",
                ),
                None,
            )
            .await;
        Ok(())
    }
}
