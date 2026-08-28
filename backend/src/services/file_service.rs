use crate::auth::audit::record_audit_log;
use crate::auth::permissions::{check_permission, PermissionAction};
use crate::auth::AuthenticatedUser;
use crate::domain::policy::{resolve_destination_permissions, PermissionInheritanceMode};
use crate::domain::{DirectoryListing, FileKind, FileMetadata, VfsPath};
use crate::errors::{AppError, VfsError};
use crate::state::AppState;
use crate::transfer::WsEvent;

pub struct FileService;

impl FileService {
    /// List files and directories in a given path for a connection with permission check, filtering, and sorting
    pub async fn list_directory(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        raw_path: Option<String>,
        show_hidden_opt: Option<bool>,
        sort_field_opt: Option<&str>,
        sort_order_opt: Option<&str>,
    ) -> Result<DirectoryListing, AppError> {
        check_permission(&state.db, user, connection_id, PermissionAction::Read).await?;

        let provider = state.get_provider(connection_id).await.ok_or_else(|| {
            VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
        })?;

        let path_str = raw_path.unwrap_or_else(|| "/".to_string());
        let vfs_path = VfsPath::new(connection_id, path_str)?;

        let mut entries = provider.list(&vfs_path).await?;

        // 1. Filter hidden files if not requested
        let show_hidden = match show_hidden_opt {
            Some(val) => val,
            None => {
                if let Some(sys_val) =
                    crate::services::settings_service::SettingsService::get_system_setting(
                        state,
                        "show_hidden_default",
                    )
                    .await
                {
                    sys_val == "true"
                } else {
                    state.config.filesystem.show_hidden_default
                }
            }
        };
        if !show_hidden {
            entries.retain(|e| !e.is_hidden);
        }

        // 2. Sort entries (directories first, then by field)
        let sort_field = sort_field_opt.unwrap_or("name");
        let is_desc = sort_order_opt == Some("desc");

        entries.sort_by(|a, b| {
            let cmp = match (a.kind, b.kind) {
                (FileKind::Directory, FileKind::File) => std::cmp::Ordering::Less,
                (FileKind::File, FileKind::Directory) => std::cmp::Ordering::Greater,
                _ => match sort_field {
                    "size" => a.size.unwrap_or(0).cmp(&b.size.unwrap_or(0)),
                    "date" => a.modified_at.cmp(&b.modified_at),
                    "type" => {
                        let ext_a = a.name.split('.').next_back().unwrap_or("");
                        let ext_b = b.name.split('.').next_back().unwrap_or("");
                        ext_a.to_lowercase().cmp(&ext_b.to_lowercase())
                    }
                    _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                },
            };
            if is_desc {
                cmp.reverse()
            } else {
                cmp
            }
        });

        // 3. Truncate to maximum configured directory entries
        let max_entries = state.config.limits.max_directory_entries;
        let total = entries.len();
        if entries.len() > max_entries {
            entries.truncate(max_entries);
        }

        Ok(DirectoryListing {
            path: vfs_path.path,
            connection_id: connection_id.to_string(),
            entries,
            total_count: total,
            next_cursor: None,
        })
    }

    /// Retrieve detailed metadata for a file or directory
    pub async fn stat_file(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        raw_path: &str,
    ) -> Result<FileMetadata, AppError> {
        check_permission(&state.db, user, connection_id, PermissionAction::Read).await?;

        let provider = state.get_provider(connection_id).await.ok_or_else(|| {
            VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
        })?;

        let vfs_path = VfsPath::new(connection_id, raw_path)?;
        let meta = provider.stat(&vfs_path).await?;
        Ok(meta)
    }

    /// Create a new empty file or save file content with permission resolution and audit logging
    pub async fn create_or_write_file(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        raw_path: &str,
        content: Vec<u8>,
        expected_etag: Option<&str>,
    ) -> Result<FileMetadata, AppError> {
        check_permission(&state.db, user, connection_id, PermissionAction::Write).await?;
        check_permission(&state.db, user, connection_id, PermissionAction::Create).await?;

        let provider = state.get_provider(connection_id).await.ok_or_else(|| {
            VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
        })?;

        let vfs_path = VfsPath::new(connection_id, raw_path)?;

        // Optimistic concurrency check (ETag)
        if let Some(expected) = expected_etag {
            if let Ok(existing_meta) = provider.stat(&vfs_path).await {
                if !existing_meta.etag.is_empty() && existing_meta.etag != expected {
                    return Err(AppError::Conflict(
                        "File has been modified by another process (ETag mismatch)".into(),
                    ));
                }
            }
        }

        // Permission inheritance
        let resolved_perms = resolve_destination_permissions(
            &provider,
            &vfs_path,
            false,
            PermissionInheritanceMode::InheritExistingOrParent,
        )
        .await;

        let cursor = std::io::Cursor::new(content);
        provider.write_stream(&vfs_path, Box::new(cursor)).await?;

        if let Some(perms) = resolved_perms {
            let _ = provider.set_permissions(&vfs_path, &perms).await;
        }

        let meta = provider.stat(&vfs_path).await?;

        // Audit log and real-time event
        record_audit_log(
            &state.db,
            Some(&user.id),
            "file_write",
            Some(connection_id),
            Some(&vfs_path.path),
            "success",
            None,
            Some(&format!("Bytes written: {}", meta.size)),
        )
        .await;

        state.transfer_manager.broadcast_event(WsEvent::FileChange {
            connection_id: connection_id.to_string(),
            path: vfs_path.path.clone(),
            action: "write".to_string(),
        });

        Ok(meta)
    }

    /// Create a new directory with permission inheritance and audit logging
    pub async fn create_directory(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        raw_path: &str,
    ) -> Result<FileMetadata, AppError> {
        check_permission(&state.db, user, connection_id, PermissionAction::Create).await?;

        let provider = state.get_provider(connection_id).await.ok_or_else(|| {
            VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
        })?;

        let vfs_path = VfsPath::new(connection_id, raw_path)?;

        let resolved_perms = resolve_destination_permissions(
            &provider,
            &vfs_path,
            true,
            PermissionInheritanceMode::InheritParent,
        )
        .await;

        provider.create_dir(&vfs_path).await?;

        if let Some(perms) = resolved_perms {
            let _ = provider.set_permissions(&vfs_path, &perms).await;
        }

        let meta = provider.stat(&vfs_path).await?;

        record_audit_log(
            &state.db,
            Some(&user.id),
            "mkdir",
            Some(connection_id),
            Some(&vfs_path.path),
            "success",
            None,
            None,
        )
        .await;

        state.transfer_manager.broadcast_event(WsEvent::FileChange {
            connection_id: connection_id.to_string(),
            path: vfs_path.path.clone(),
            action: "create".to_string(),
        });

        Ok(meta)
    }

    /// Delete a file or directory with audit logging
    pub async fn delete_entry(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        raw_path: &str,
    ) -> Result<(), AppError> {
        check_permission(&state.db, user, connection_id, PermissionAction::Delete).await?;

        let provider = state.get_provider(connection_id).await.ok_or_else(|| {
            VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
        })?;

        let vfs_path = VfsPath::new(connection_id, raw_path)?;
        provider.delete(&vfs_path).await?;

        record_audit_log(
            &state.db,
            Some(&user.id),
            "delete",
            Some(connection_id),
            Some(&vfs_path.path),
            "success",
            None,
            None,
        )
        .await;

        state.transfer_manager.broadcast_event(WsEvent::FileChange {
            connection_id: connection_id.to_string(),
            path: vfs_path.path.clone(),
            action: "delete".to_string(),
        });

        Ok(())
    }

    /// Rename / move a file within the same connection with audit logging
    pub async fn rename_entry(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        from_raw: &str,
        to_raw: &str,
    ) -> Result<(), AppError> {
        check_permission(&state.db, user, connection_id, PermissionAction::Write).await?;
        check_permission(&state.db, user, connection_id, PermissionAction::Delete).await?;

        let provider = state.get_provider(connection_id).await.ok_or_else(|| {
            VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
        })?;

        let from_vfs = VfsPath::new(connection_id, from_raw)?;
        let to_vfs = VfsPath::new(connection_id, to_raw)?;

        provider.rename(&from_vfs, &to_vfs).await?;

        record_audit_log(
            &state.db,
            Some(&user.id),
            "rename",
            Some(connection_id),
            Some(&from_vfs.path),
            "success",
            None,
            Some(&format!("Renamed to: {}", to_vfs.path)),
        )
        .await;

        state.transfer_manager.broadcast_event(WsEvent::FileChange {
            connection_id: connection_id.to_string(),
            path: to_vfs.path.clone(),
            action: "rename".to_string(),
        });

        Ok(())
    }

    /// Change permissions (mode) for a path
    pub async fn chmod(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        raw_path: &str,
        mode: u32,
    ) -> Result<(), AppError> {
        check_permission(&state.db, user, connection_id, PermissionAction::Write).await?;

        let provider = state.get_provider(connection_id).await.ok_or_else(|| {
            VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
        })?;

        let vfs_path = VfsPath::new(connection_id, raw_path)?;
        let mode_str = format!("{:04o}", mode);
        provider.set_permissions(&vfs_path, &mode_str).await?;

        record_audit_log(
            &state.db,
            Some(&user.id),
            "chmod",
            Some(connection_id),
            Some(&vfs_path.path),
            "success",
            None,
            Some(&format!("Mode changed to: {:04o}", mode)),
        )
        .await;

        Ok(())
    }
}
