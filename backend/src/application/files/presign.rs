use crate::domain::{Actor, ConnectionId, FileMetadata, VfsPath};
use crate::errors::{AppError, VfsError};
use crate::ports::{
    authorization::{Authorization, FileAction},
    effects::{FileAccessEffects, FileMutationEffects},
    filesystem::FileSystemResolver,
};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct PresignCommand {
    pub connection: ConnectionId,
    pub path: String,
    pub expire_secs: u64,
}

#[derive(Clone)]
pub struct PresignDownload {
    authorization: Arc<dyn Authorization>,
    filesystem: Arc<dyn FileSystemResolver>,
    effects: Arc<dyn FileAccessEffects>,
}

impl PresignDownload {
    pub fn new(
        authorization: Arc<dyn Authorization>,
        filesystem: Arc<dyn FileSystemResolver>,
        effects: Arc<dyn FileAccessEffects>,
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
        command: PresignCommand,
    ) -> Result<String, AppError> {
        self.authorization
            .authorize(actor, &command.connection, FileAction::Read)
            .await?;
        let provider = self.filesystem.resolve(&command.connection).await?;
        let presign = provider.as_presign().ok_or_else(|| {
            VfsError::NotSupported(format!(
                "Pre-signed download not supported by '{}'",
                command.connection.as_str()
            ))
        })?;
        let path = VfsPath::new(command.connection.as_str(), command.path)?;
        let url = presign
            .presign_read_url(
                &path,
                Duration::from_secs(command.expire_secs.clamp(60, 86400)),
            )
            .await?;
        self.effects
            .accessed(
                actor,
                &command.connection,
                &path.path,
                "presign_download",
                None,
            )
            .await;
        Ok(url)
    }
}

#[derive(Clone)]
pub struct PresignUpload {
    authorization: Arc<dyn Authorization>,
    filesystem: Arc<dyn FileSystemResolver>,
    effects: Arc<dyn FileAccessEffects>,
}

impl PresignUpload {
    pub fn new(
        authorization: Arc<dyn Authorization>,
        filesystem: Arc<dyn FileSystemResolver>,
        effects: Arc<dyn FileAccessEffects>,
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
        command: PresignCommand,
    ) -> Result<String, AppError> {
        self.authorization
            .authorize(actor, &command.connection, FileAction::Write)
            .await?;
        let provider = self.filesystem.resolve(&command.connection).await?;
        let presign = provider.as_presign().ok_or_else(|| {
            VfsError::NotSupported(format!(
                "Pre-signed upload not supported by '{}'",
                command.connection.as_str()
            ))
        })?;
        let path = VfsPath::new(command.connection.as_str(), command.path)?;
        let url = presign
            .presign_write_url(
                &path,
                Duration::from_secs(command.expire_secs.clamp(60, 86400)),
            )
            .await?;
        self.effects
            .accessed(
                actor,
                &command.connection,
                &path.path,
                "presign_upload",
                None,
            )
            .await;
        Ok(url)
    }
}

#[derive(Debug, Clone)]
pub struct CompletePresignedCommand {
    pub connection: ConnectionId,
    pub path: String,
    pub expected_size: Option<u64>,
    pub expected_checksum: Option<String>,
}

#[derive(Clone)]
pub struct CompletePresigned {
    authorization: Arc<dyn Authorization>,
    filesystem: Arc<dyn FileSystemResolver>,
    effects: Arc<dyn FileMutationEffects>,
}

impl CompletePresigned {
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
        command: CompletePresignedCommand,
    ) -> Result<FileMetadata, AppError> {
        self.authorization
            .authorize(actor, &command.connection, FileAction::Write)
            .await?;
        let provider = self.filesystem.resolve(&command.connection).await?;
        let path = VfsPath::new(command.connection.as_str(), command.path)?;
        let metadata = provider.stat(&path).await.map_err(|error| match error {
            VfsError::NotFound(_) => {
                AppError::NotFound(format!("Uploaded file not found at '{}'", path.path))
            }
            other => AppError::from(other),
        })?;

        if let Some(expected) = command.expected_size {
            if metadata.size != expected {
                return Err(AppError::BadRequest(format!(
                    "size mismatch expected {} found {}",
                    expected, metadata.size
                )));
            }
        }
        if let Some(expected) = command.expected_checksum {
            let clean_expected = expected.trim().trim_matches('"');
            let clean_etag = metadata.etag.trim().trim_matches('"');
            if !clean_etag.is_empty() && !clean_etag.eq_ignore_ascii_case(clean_expected) {
                return Err(AppError::BadRequest(format!(
                    "checksum mismatch expected '{}' found '{}'",
                    clean_expected, clean_etag
                )));
            }
        }

        self.effects
            .invalidate(&command.connection, &path.path)
            .await;
        self.effects
            .file_changed(
                actor,
                &command.connection,
                &path.path,
                "presign_upload_complete",
                "upload",
                Some(format!("size={}, etag={:?}", metadata.size, metadata.etag)),
            )
            .await;
        Ok(metadata)
    }
}
