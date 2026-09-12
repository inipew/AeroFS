use super::FileApplicationService;
use crate::auth::UserInfo;
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
        Self { authorization, filesystem, effects }
    }

    pub async fn execute(&self, actor: &Actor, command: PresignCommand) -> Result<String, AppError> {
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
            .presign_read_url(&path, Duration::from_secs(command.expire_secs.clamp(60, 86400)))
            .await?;
        self.effects
            .accessed(actor, &command.connection, &path.path, "presign_download", None)
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
        Self { authorization, filesystem, effects }
    }

    pub async fn execute(&self, actor: &Actor, command: PresignCommand) -> Result<String, AppError> {
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
            .presign_write_url(&path, Duration::from_secs(command.expire_secs.clamp(60, 86400)))
            .await?;
        self.effects
            .accessed(actor, &command.connection, &path.path, "presign_upload", None)
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
        Self { authorization, filesystem, effects }
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

        self.effects.invalidate(&command.connection, &path.path).await;
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

impl FileApplicationService {
    pub async fn presign_download_typed(
        &self,
        user: &UserInfo,
        connection: &ConnectionId,
        raw_path: String,
        expire_secs: Option<u64>,
    ) -> Result<String, AppError> {
        use crate::auth::permissions::{check_permission, PermissionAction};
        check_permission(&self.db, user, connection.as_str(), PermissionAction::Read).await?;
        let provider = self.registry.get(connection.as_str()).await.ok_or_else(|| {
            VfsError::ConnectionError(format!("Connection '{}' not found", connection.as_str()))
        })?;
        let presign = provider.as_presign().ok_or_else(|| {
            VfsError::NotSupported(format!("Pre-signed download not supported by '{}'", connection.as_str()))
        })?;
        let vfs_path = VfsPath::new(connection.as_str(), raw_path.clone())?;
        let ttl = Duration::from_secs(expire_secs.unwrap_or(3600).clamp(60, 86400));
        let url = presign.presign_read_url(&vfs_path, ttl).await?;
        crate::auth::audit::record_audit_log(
            &self.db,
            Some(&user.id),
            "presign_download",
            Some(connection.as_str()),
            Some(&raw_path),
            "success",
            None,
            None,
        )
        .await;
        Ok(url)
    }

    pub async fn presign_upload_typed(
        &self,
        user: &UserInfo,
        connection: &ConnectionId,
        raw_path: String,
        expire_secs: Option<u64>,
    ) -> Result<String, AppError> {
        use crate::auth::permissions::{check_permission, PermissionAction};
        check_permission(&self.db, user, connection.as_str(), PermissionAction::Write).await?;
        let provider = self.registry.get(connection.as_str()).await.ok_or_else(|| {
            VfsError::ConnectionError(format!("Connection '{}' not found", connection.as_str()))
        })?;
        let presign = provider.as_presign().ok_or_else(|| {
            VfsError::NotSupported(format!("Pre-signed upload not supported by '{}'", connection.as_str()))
        })?;
        let vfs_path = VfsPath::new(connection.as_str(), raw_path.clone())?;
        let ttl = Duration::from_secs(expire_secs.unwrap_or(3600).clamp(60, 86400));
        let url = presign.presign_write_url(&vfs_path, ttl).await?;
        crate::auth::audit::record_audit_log(
            &self.db,
            Some(&user.id),
            "presign_upload",
            Some(connection.as_str()),
            Some(&raw_path),
            "success",
            None,
            None,
        )
        .await;
        Ok(url)
    }

    pub async fn complete_presigned_typed(
        &self,
        user: &UserInfo,
        connection: &ConnectionId,
        raw_path: String,
        expected_size: Option<u64>,
        expected_checksum: Option<String>,
    ) -> Result<FileMetadata, AppError> {
        use crate::auth::permissions::{check_permission, PermissionAction};
        check_permission(&self.db, user, connection.as_str(), PermissionAction::Write).await?;
        let provider = self.registry.get(connection.as_str()).await.ok_or_else(|| {
            VfsError::ConnectionError(format!("Connection '{}' not found", connection.as_str()))
        })?;
        let vfs_path = VfsPath::new(connection.as_str(), raw_path.clone())?;
        let meta = provider.stat(&vfs_path).await.map_err(|e| match e {
            VfsError::NotFound(_) => AppError::NotFound(format!("Uploaded file not found at '{}'", vfs_path.path)),
            other => AppError::from(other),
        })?;
        if let Some(exp_size) = expected_size {
            if meta.size != exp_size {
                return Err(AppError::BadRequest(format!("size mismatch expected {} found {}", exp_size, meta.size)));
            }
        }
        if let Some(exp_chk) = expected_checksum {
            let clean_exp = exp_chk.trim().trim_matches('"');
            let clean_etag = meta.etag.trim().trim_matches('"');
            if !clean_etag.is_empty() && !clean_etag.eq_ignore_ascii_case(clean_exp) {
                return Err(AppError::BadRequest(format!("checksum mismatch expected '{}' found '{}'", clean_exp, clean_etag)));
            }
        }
        self.metadata_cache.invalidate(connection.as_str(), &raw_path).await;
        crate::auth::audit::record_audit_log(
            &self.db,
            Some(&user.id),
            "presign_upload_complete",
            Some(connection.as_str()),
            Some(&raw_path),
            "success",
            None,
            Some(&format!("size={}, etag={:?}", meta.size, meta.etag)),
        )
        .await;
        let _ = self.event_journal.append(
            crate::events::DomainEvent::file_change(connection.as_str(), &vfs_path.path, "upload"),
            None,
        ).await;
        Ok(meta)
    }
}
