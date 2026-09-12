use crate::domain::{Actor, ConnectionId, FileMetadata, PermissionInheritanceMode, VfsPath};
use crate::errors::{AppError, VfsError};
use crate::ports::{
    authorization::{Authorization, FileAction},
    effects::FileMutationEffects,
    filesystem::FileSystemResolver,
    mutation::MutationCoordinator,
    settings::FileSettings,
};
use std::io::Cursor;
use std::sync::Arc;
use uuid::Uuid;

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
    mutations: Arc<dyn MutationCoordinator>,
}

impl WriteFile {
    pub fn new(
        authorization: Arc<dyn Authorization>,
        filesystem: Arc<dyn FileSystemResolver>,
        settings: Arc<dyn FileSettings>,
        effects: Arc<dyn FileMutationEffects>,
        mutations: Arc<dyn MutationCoordinator>,
    ) -> Self {
        Self {
            authorization,
            filesystem,
            settings,
            effects,
            mutations,
        }
    }

    pub async fn execute(
        &self,
        actor: &Actor,
        command: WriteFileCommand,
    ) -> Result<FileMetadata, AppError> {
        use crate::domain::policy::resolve_destination_permissions_strict;

        self.authorization
            .authorize(actor, &command.connection, FileAction::Write)
            .await?;
        self.authorization
            .authorize(actor, &command.connection, FileAction::Create)
            .await?;

        let provider = self.filesystem.resolve(&command.connection).await?;
        let path = VfsPath::new(command.connection.as_str(), command.path.clone())?;

        let _mutation_guard = self
            .mutations
            .try_acquire(&command.connection, &path.path)
            .await?;

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
        let permissions = if capabilities.permissions {
            resolve_destination_permissions_strict(
                &provider,
                &path,
                false,
                PermissionInheritanceMode::InheritExistingOrParent,
            )
            .await?
        } else {
            None
        };
        let content = command.content;
        let mut permissions_applied_before_commit = false;

        if capabilities.atomic_rename {
            let temporary = VfsPath::new(
                command.connection.as_str(),
                format!("{}.aerofs.tmp-{}", path.path, Uuid::new_v4()),
            )?;
            if provider
                .write_stream(&temporary, Box::new(Cursor::new(content.clone())))
                .await
                .is_ok()
            {
                if let Some(ref permissions) = permissions {
                    if let Err(error) = provider.set_permissions(&temporary, permissions).await {
                        let _ = provider.delete(&temporary).await;
                        return Err(error.into());
                    }
                    permissions_applied_before_commit = true;
                }
                if let Err(error) = provider.rename(&temporary, &path).await {
                    tracing::warn!(
                        "Atomic rename failed {}→{}: {}. Fallback direct",
                        temporary.path,
                        path.path,
                        error
                    );
                    let _ = provider.delete(&temporary).await;
                    permissions_applied_before_commit = false;
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

        if !permissions_applied_before_commit {
            if let Some(ref permissions) = permissions {
                if let Err(error) = provider.set_permissions(&path, permissions).await {
                    return Err(AppError::Internal(anyhow::anyhow!(
                        "File '{}' content was written, but applying inherited permissions '{}' failed: {}. Filesystem mutation committed; recovery required",
                        path.path,
                        permissions,
                        error
                    )));
                }
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
            .await?;
        Ok(metadata)
    }
}
