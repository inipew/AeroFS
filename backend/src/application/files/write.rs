use super::FileApplicationService;
use crate::auth::UserInfo;
use crate::domain::{Actor, ConnectionId, FileMetadata, PermissionInheritanceMode, VfsPath};
use crate::errors::{AppError, VfsError};
use crate::ports::{
    authorization::{Authorization, FileAction},
    effects::FileMutationEffects,
    filesystem::FileSystemResolver,
    settings::FileSettings,
};
use std::io::Cursor;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct WriteFileCommand {
    pub connection: ConnectionId,
    pub path: String,
    pub content: Vec<u8>,
    pub expected_etag: Option<String>,
    pub create_only: bool,
}

#[derive(Clone)]
pub struct WriteFile {
    authorization: Arc<dyn Authorization>,
    filesystem: Arc<dyn FileSystemResolver>,
    settings: Arc<dyn FileSettings>,
    effects: Arc<dyn FileMutationEffects>,
}

impl WriteFile {
    pub fn new(
        authorization: Arc<dyn Authorization>,
        filesystem: Arc<dyn FileSystemResolver>,
        settings: Arc<dyn FileSettings>,
        effects: Arc<dyn FileMutationEffects>,
    ) -> Self {
        Self {
            authorization,
            filesystem,
            settings,
            effects,
        }
    }

    pub async fn execute(
        &self,
        actor: &Actor,
        command: WriteFileCommand,
    ) -> Result<FileMetadata, AppError> {
        use crate::domain::policy::resolve_destination_permissions;

        self.authorization
            .authorize(actor, &command.connection, FileAction::Write)
            .await?;
        self.authorization
            .authorize(actor, &command.connection, FileAction::Create)
            .await?;

        let provider = self.filesystem.resolve(&command.connection).await?;
        let path = VfsPath::new(command.connection.as_str(), command.path.clone())?;

        if command.create_only {
            match provider.stat(&path).await {
                Ok(_) => {
                    return Err(AppError::Vfs(VfsError::AlreadyExists(format!(
                        "File '{}' already exists",
                        path.path
                    ))));
                }
                Err(VfsError::NotFound(_)) => {}
                Err(error) => return Err(error.into()),
            }
        }

        if !command.content.is_empty() {
            let max_editable_size = self.settings.max_editable_size().await?;
            if command.content.len() as u64 > max_editable_size {
                return Err(AppError::PayloadTooLarge(format!(
                    "File content length ({} bytes) exceeds maximum editable size of {} bytes",
                    command.content.len(),
                    max_editable_size
                )));
            }
        }

        if let Some(expected) = command.expected_etag.as_deref() {
            let existing = provider.stat(&path).await.map_err(|error| match error {
                VfsError::NotFound(_) => AppError::PreconditionFailed(format!(
                    "Target file '{}' does not exist for If-Match precondition",
                    path.path
                )),
                other => AppError::from(other),
            })?;
            let clean_expected = expected.trim().trim_matches('"');
            let clean_actual = existing.etag.trim().trim_matches('"');
            if clean_expected != clean_actual && expected != "*" {
                return Err(AppError::PreconditionFailed(format!(
                    "File was modified externally. Expected ETag: {}, Current ETag: {}",
                    clean_expected, clean_actual
                )));
            }
        }

        let capabilities = provider.capabilities();
        let permissions = resolve_destination_permissions(
            &provider,
            &path,
            false,
            PermissionInheritanceMode::InheritExistingOrParent,
        )
        .await;
        let content = command.content;

        if capabilities.atomic_rename {
            let temporary = VfsPath::new(
                command.connection.as_str(),
                format!("{}.aerofs.tmp", path.path),
            )?;
            if provider
                .write_stream(&temporary, Box::new(Cursor::new(content.clone())))
                .await
                .is_ok()
            {
                if capabilities.permissions {
                    if let Some(ref permissions) = permissions {
                        let _ = provider.set_permissions(&temporary, permissions).await;
                    }
                }
                if let Err(error) = provider.rename(&temporary, &path).await {
                    tracing::warn!(
                        "Atomic rename failed {}→{}: {}. Fallback direct",
                        temporary.path,
                        path.path,
                        error
                    );
                    let _ = provider.delete(&temporary).await;
                    provider
                        .write_stream(&path, Box::new(Cursor::new(content)))
                        .await?;
                }
            } else {
                provider
                    .write_stream(&path, Box::new(Cursor::new(content)))
                    .await?;
            }
        } else {
            provider
                .write_stream(&path, Box::new(Cursor::new(content)))
                .await?;
        }

        if capabilities.permissions {
            if let Some(ref permissions) = permissions {
                let _ = provider.set_permissions(&path, permissions).await;
            }
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
                "FILE_WRITE",
                "write",
                Some(format!("Bytes written: {}", metadata.size)),
            )
            .await;
        Ok(metadata)
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
    pub async fn create_file_typed(
        &self,
        user: &UserInfo,
        connection: &ConnectionId,
        raw_path: String,
    ) -> Result<FileMetadata, AppError> {
        self.write_file
            .execute(
                &actor_from_user(user),
                WriteFileCommand {
                    connection: connection.clone(),
                    path: raw_path,
                    content: Vec::new(),
                    expected_etag: None,
                    create_only: true,
                },
            )
            .await
    }

    pub async fn create_or_write_typed(
        &self,
        user: &UserInfo,
        connection: &ConnectionId,
        raw_path: String,
        content: Vec<u8>,
        expected_etag: Option<String>,
    ) -> Result<FileMetadata, AppError> {
        self.write_file
            .execute(
                &actor_from_user(user),
                WriteFileCommand {
                    connection: connection.clone(),
                    path: raw_path,
                    content,
                    expected_etag,
                    create_only: false,
                },
            )
            .await
    }

    pub async fn write_typed(
        &self,
        user: &UserInfo,
        connection: &ConnectionId,
        path: String,
        content: Vec<u8>,
    ) -> Result<(), AppError> {
        self.create_or_write_typed(user, connection, path, content, None)
            .await
            .map(|_| ())
    }
}
