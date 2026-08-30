use super::FileApplicationService;
use crate::auth::UserInfo;
use crate::domain::{VfsPath, FileMetadata};
use crate::errors::{AppError, VfsError};

#[derive(Debug, Clone)]
pub struct ReadOptions {
    pub path: String,
    pub download: bool,
    pub range: Option<String>,
}

impl FileApplicationService {
    pub async fn stat_typed(
        &self,
        user: &UserInfo,
        connection: &crate::domain::ConnectionId,
        raw_path: String,
    ) -> Result<FileMetadata, AppError> {
        use crate::auth::permissions::{check_permission, PermissionAction};
        check_permission(&self.db, user, connection.as_str(), PermissionAction::Read).await?;
        let provider = self.registry.get(connection.as_str()).await.ok_or_else(|| {
            VfsError::ConnectionError(format!("Connection '{}' not found", connection.as_str()))
        })?;
        let vfs_path = VfsPath::new(connection.as_str(), raw_path.clone())?;
        self.metadata_cache.get_or_fetch(connection.as_str(), &raw_path, || async {
            let meta = provider.stat(&vfs_path).await?;
            Ok(meta)
        }).await
    }

    pub async fn read_typed(
        &self,
        user: &crate::auth::UserInfo,
        connection: &crate::domain::ConnectionId,
        opts: ReadOptions,
    ) -> Result<(Vec<u8>, VfsPath), AppError> {
        // stat path for now — full streaming handled at API layer via provider
        let meta = self.stat_typed(user, connection, opts.path.clone()).await?;
        Ok((vec![], VfsPath::new(connection.as_str(), meta.path)?))
    }
}
