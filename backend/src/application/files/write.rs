use super::FileApplicationService;
use crate::auth::UserInfo;
use crate::domain::{VfsPath, PermissionInheritanceMode, FileMetadata};
use crate::errors::{AppError, VfsError};
use std::io::Cursor;

impl FileApplicationService {
    /// Owned write — no AppState, explicit ports (Phase 3.2).
    pub async fn create_or_write_typed(
        &self,
        user: &UserInfo,
        connection: &crate::domain::ConnectionId,
        raw_path: String,
        content: Vec<u8>,
        expected_etag: Option<String>,
    ) -> Result<FileMetadata, AppError> {
        use crate::auth::permissions::{check_permission, PermissionAction};
        use crate::domain::policy::resolve_destination_permissions;

        check_permission(&self.db, user, connection.as_str(), PermissionAction::Write).await?;
        check_permission(&self.db, user, connection.as_str(), PermissionAction::Create).await?;

        let provider = self.registry.get(connection.as_str()).await.ok_or_else(|| {
            VfsError::ConnectionError(format!("Connection '{}' not found", connection.as_str()))
        })?;

        let vfs_path = VfsPath::new(connection.as_str(), raw_path.clone())?;

        if !content.is_empty() {
            let max_editable_bytes: u64 = sqlx::query_scalar::<_, String>(
                "SELECT value FROM system_settings WHERE key = 'max_editable_size'",
            )
            .fetch_optional(&self.db)
            .await
            .ok()
            .flatten()
            .and_then(|v| v.parse().ok())
            .unwrap_or(self.config.limits.max_editable_size);
            if content.len() as u64 > max_editable_bytes {
                return Err(AppError::PayloadTooLarge(format!(
                    "File content length ({} bytes) exceeds maximum editable size of {} bytes",
                    content.len(),
                    max_editable_bytes
                )));
            }
        }

        if let Some(expected) = expected_etag.as_deref() {
            let existing_meta = provider.stat(&vfs_path).await.map_err(|e| match e {
                VfsError::NotFound(_) => AppError::PreconditionFailed(format!(
                    "Target file '{}' does not exist for If-Match precondition",
                    vfs_path.path
                )),
                other => AppError::from(other),
            })?;
            let clean_expected = expected.trim().trim_matches('"');
            let clean_actual = existing_meta.etag.trim().trim_matches('"');
            if clean_expected != clean_actual && expected != "*" {
                return Err(AppError::PreconditionFailed(format!(
                    "File was modified externally. Expected ETag: {}, Current ETag: {}",
                    clean_expected, clean_actual
                )));
            }
        }

        let caps = provider.capabilities();
        let target_perms = resolve_destination_permissions(
            &provider,
            &vfs_path,
            false,
            PermissionInheritanceMode::InheritExistingOrParent,
        )
        .await;

        if caps.atomic_rename {
            let tmp_vfs = VfsPath::new(connection.as_str(), format!("{}.aerofs.tmp", vfs_path.path))?;
            let cursor = Cursor::new(content.clone());
            if provider.write_stream(&tmp_vfs, Box::new(cursor)).await.is_ok() {
                if caps.permissions {
                    if let Some(ref perms) = target_perms {
                        let _ = provider.set_permissions(&tmp_vfs, perms).await;
                    }
                }
                if let Err(rename_err) = provider.rename(&tmp_vfs, &vfs_path).await {
                    tracing::warn!("Atomic rename failed {}→{}: {}. Fallback direct", tmp_vfs.path, vfs_path.path, rename_err);
                    let _ = provider.delete(&tmp_vfs).await;
                    let fallback = Cursor::new(content);
                    provider.write_stream(&vfs_path, Box::new(fallback)).await?;
                    if caps.permissions {
                        if let Some(ref perms) = target_perms { let _ = provider.set_permissions(&vfs_path, perms).await; }
                    }
                } else if caps.permissions {
                    if let Some(ref perms) = target_perms { let _ = provider.set_permissions(&vfs_path, perms).await; }
                }
            } else {
                let fallback = Cursor::new(content);
                provider.write_stream(&vfs_path, Box::new(fallback)).await?;
                if caps.permissions { if let Some(ref perms) = target_perms { let _ = provider.set_permissions(&vfs_path, perms).await; } }
            }
        } else {
            let cursor = Cursor::new(content);
            provider.write_stream(&vfs_path, Box::new(cursor)).await?;
            if caps.permissions { if let Some(ref perms) = target_perms { let _ = provider.set_permissions(&vfs_path, perms).await; } }
        }

        let meta = provider.stat(&vfs_path).await?;
        self.metadata_cache.invalidate(connection.as_str(), &raw_path).await;
        crate::auth::audit::record_audit_log(
            &self.db,
            Some(&user.id),
            "FILE_WRITE",
            Some(connection.as_str()),
            Some(&vfs_path.path),
            "SUCCESS",
            None,
            Some(&format!("Bytes written: {}", meta.size)),
        )
        .await;
        let _ = self.event_journal.append(crate::events::DomainEvent::file_change(connection.as_str(), &vfs_path.path, "write"), None).await;
        Ok(meta)
    }

    pub async fn write_typed(
        &self,
        user: &crate::auth::UserInfo,
        connection: &crate::domain::ConnectionId,
        path: String,
        content: Vec<u8>,
    ) -> Result<(), AppError> {
        self.create_or_write_typed(user, connection, path, content, None).await.map(|_| ())
    }
}
