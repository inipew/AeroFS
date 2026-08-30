use super::FileApplicationService;
use crate::auth::UserInfo;
use crate::domain::{VfsPath, FileMetadata, PermissionInheritanceMode};
use crate::errors::{AppError, VfsError};
use std::sync::Arc;
use tokio::task::JoinSet;

impl FileApplicationService {
    pub async fn create_directory_typed(
        &self,
        user: &UserInfo,
        connection: &crate::domain::ConnectionId,
        raw_path: String,
    ) -> Result<FileMetadata, AppError> {
        use crate::auth::permissions::{check_permission, PermissionAction};
        use crate::domain::policy::resolve_destination_permissions;
        check_permission(&self.db, user, connection.as_str(), PermissionAction::Create).await?;
        let provider = self.registry.get(connection.as_str()).await.ok_or_else(|| {
            VfsError::ConnectionError(format!("Connection '{}' not found", connection.as_str()))
        })?;
        let vfs_path = VfsPath::new(connection.as_str(), raw_path.clone())?;
        let resolved_perms = resolve_destination_permissions(&provider, &vfs_path, true, PermissionInheritanceMode::InheritParent).await;
        provider.create_dir(&vfs_path).await?;
        if let Some(perms) = resolved_perms { let _ = provider.set_permissions(&vfs_path, &perms).await; }
        let meta = provider.stat(&vfs_path).await?;
        self.metadata_cache.invalidate(connection.as_str(), &raw_path).await;
        crate::auth::audit::record_audit_log(&self.db, Some(&user.id), "FILE_MKDIR", Some(connection.as_str()), Some(&vfs_path.path), "SUCCESS", None, None).await;
        let _ = self.event_journal.append(crate::events::DomainEvent::file_change(connection.as_str(), &vfs_path.path, "create"), None).await;
        Ok(meta)
    }

    pub async fn delete_files_typed(
        &self,
        user: &UserInfo,
        connection: &crate::domain::ConnectionId,
        paths: Vec<String>,
    ) -> Result<(Vec<String>, Vec<(String, String)>), AppError> {
        use crate::auth::permissions::{check_permission, PermissionAction};
        check_permission(&self.db, user, connection.as_str(), PermissionAction::Delete).await?;
        let provider = self.registry.get(connection.as_str()).await.ok_or_else(|| {
            VfsError::ConnectionError(format!("Connection '{}' not found", connection.as_str()))
        })?;
        let mut succeeded = Vec::new();
        let mut failed = Vec::new();
        let mut tasks = JoinSet::new();
        let sem = Arc::new(tokio::sync::Semaphore::new(8));
        for raw_path in paths {
            let p_clone = provider.clone();
            let conn_str = connection.to_string();
            let sem_clone = sem.clone();
            tasks.spawn(async move {
                let _permit = sem_clone.acquire().await.map_err(|_| VfsError::IoError("Semaphore closed".into()))?;
                let vfs_path = VfsPath::new(&conn_str, &raw_path)?;
                let res = p_clone.delete(&vfs_path).await;
                Ok::<_, VfsError>((raw_path, res))
            });
        }
        while let Some(join_res) = tasks.join_next().await {
            if let Ok(Ok((path, del_res))) = join_res {
                match del_res {
                    Ok(_) => {
                        self.metadata_cache.invalidate_prefix(connection.as_str(), &path).await;
                        crate::auth::audit::record_audit_log(&self.db, Some(&user.id), "FILE_DELETE", Some(connection.as_str()), Some(&path), "SUCCESS", None, None).await;
                        let _ = self.event_journal.append(crate::events::DomainEvent::file_change(connection.as_str(), &path, "delete"), None).await;
                        succeeded.push(path);
                    }
                    Err(e) => failed.push((path, e.to_string())),
                }
            }
        }
        succeeded.sort(); failed.sort_by(|a,b| a.0.cmp(&b.0));
        Ok((succeeded, failed))
    }

    pub async fn rename_typed(
        &self,
        user: &UserInfo,
        connection: &crate::domain::ConnectionId,
        from_raw: String,
        to_raw: String,
    ) -> Result<(), AppError> {
        use crate::auth::permissions::{check_permission, PermissionAction};
        check_permission(&self.db, user, connection.as_str(), PermissionAction::Write).await?;
        check_permission(&self.db, user, connection.as_str(), PermissionAction::Delete).await?;
        let provider = self.registry.get(connection.as_str()).await.ok_or_else(|| {
            VfsError::ConnectionError(format!("Connection '{}' not found", connection.as_str()))
        })?;
        let from_vfs = VfsPath::new(connection.as_str(), from_raw.clone())?;
        let to_vfs = VfsPath::new(connection.as_str(), to_raw.clone())?;
        provider.rename(&from_vfs, &to_vfs).await?;
        self.metadata_cache.invalidate_prefix(connection.as_str(), &from_raw).await;
        self.metadata_cache.invalidate_prefix(connection.as_str(), &to_raw).await;
        crate::auth::audit::record_audit_log(&self.db, Some(&user.id), "FILE_RENAME", Some(connection.as_str()), Some(&from_vfs.path), "SUCCESS", None, Some(&format!("Renamed to: {}", to_vfs.path))).await;
        let _ = self.event_journal.append(crate::events::DomainEvent::file_rename(connection.as_str(), &from_vfs.path, &to_vfs.path), None).await;
        Ok(())
    }

    pub async fn chmod_typed(
        &self,
        user: &UserInfo,
        connection: &crate::domain::ConnectionId,
        raw_path: String,
        mode: u32,
    ) -> Result<(), AppError> {
        use crate::auth::permissions::{check_permission, PermissionAction};
        check_permission(&self.db, user, connection.as_str(), PermissionAction::Write).await?;
        let provider = self.registry.get(connection.as_str()).await.ok_or_else(|| {
            VfsError::ConnectionError(format!("Connection '{}' not found", connection.as_str()))
        })?;
        let vfs_path = VfsPath::new(connection.as_str(), raw_path.clone())?;
        let mode_str = format!("{:04o}", mode);
        provider.set_permissions(&vfs_path, &mode_str).await?;
        self.metadata_cache.invalidate(connection.as_str(), &raw_path).await;
        crate::auth::audit::record_audit_log(&self.db, Some(&user.id), "FILE_CHMOD", Some(connection.as_str()), Some(&vfs_path.path), "SUCCESS", None, Some(&format!("Mode changed to: {:04o}", mode))).await;
        let _ = self.event_journal.append(crate::events::DomainEvent::file_change(connection.as_str(), &vfs_path.path, "chmod"), None).await;
        Ok(())
    }
}
