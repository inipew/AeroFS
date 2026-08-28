use crate::auth::audit::record_audit_log;
use crate::auth::permissions::{check_permission, PermissionAction};
use crate::auth::AuthenticatedUser;
use crate::domain::policy::{resolve_destination_permissions, PermissionInheritanceMode};
use crate::domain::{DirectoryListing, FileKind, FileMetadata, VfsPath};
use crate::errors::{AppError, VfsError};
use crate::services::settings_service::SettingsService;
use crate::state::AppState;
use crate::transfer::WsEvent;
use std::io::Cursor;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

pub struct FileService;

impl FileService {
    /// List entries in a directory, applying sorting and hidden file filtering
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
                    SettingsService::get_system_setting(state, "show_hidden_default").await
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
        let sort_order = sort_order_opt.unwrap_or("asc");

        entries.sort_by(|a, b| {
            let a_is_dir = a.kind == FileKind::Directory;
            let b_is_dir = b.kind == FileKind::Directory;

            if a_is_dir != b_is_dir {
                return b_is_dir.cmp(&a_is_dir);
            }

            let cmp = match sort_field {
                "size" => {
                    let a_size = a.size.unwrap_or(0);
                    let b_size = b.size.unwrap_or(0);
                    a_size.cmp(&b_size)
                }
                "modified" => {
                    let a_mod = a.modified_at.map(|d| d.timestamp()).unwrap_or(0);
                    let b_mod = b.modified_at.map(|d| d.timestamp()).unwrap_or(0);
                    a_mod.cmp(&b_mod)
                }
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            };

            if sort_order == "desc" {
                cmp.reverse()
            } else {
                cmp
            }
        });

        let total_count = entries.len();

        Ok(DirectoryListing {
            path: vfs_path.path,
            connection_id: connection_id.to_string(),
            entries,
            total_count,
            next_cursor: None,
        })
    }

    /// Retrieve file metadata
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

        // Max editable size check if saving content
        if !content.is_empty() {
            let max_editable_bytes = if let Some(custom) =
                SettingsService::get_system_setting(state, "max_editable_size").await
            {
                custom
                    .parse()
                    .unwrap_or(state.config.limits.max_editable_size)
            } else {
                state.config.limits.max_editable_size
            };

            if content.len() as u64 > max_editable_bytes {
                return Err(AppError::PayloadTooLarge(format!(
                    "File content length ({} bytes) exceeds maximum editable size of {} bytes",
                    content.len(),
                    max_editable_bytes
                )));
            }
        }

        // Optimistic concurrency check (ETag)
        if let Some(expected) = expected_etag {
            if let Ok(existing_meta) = provider.stat(&vfs_path).await {
                let clean_expected = expected.trim().trim_matches('"');
                let clean_actual = existing_meta.etag.trim().trim_matches('"');
                if clean_expected != clean_actual && expected != "*" {
                    return Err(AppError::Conflict(format!(
                        "File was modified externally. Expected ETag: {}, Current ETag: {}",
                        clean_expected, clean_actual
                    )));
                }
            }
        }

        // Destination permission preservation: capture existing permissions or inherit from parent
        let target_perms = resolve_destination_permissions(
            &provider,
            &vfs_path,
            false,
            PermissionInheritanceMode::InheritExistingOrParent,
        )
        .await;

        // Atomic write safety: write to temporary path first then rename
        let tmp_vfs = VfsPath::new(connection_id, format!("{}.aerofs.tmp", vfs_path.path))?;
        let cursor = Cursor::new(content.clone());

        if provider
            .write_stream(&tmp_vfs, Box::new(cursor))
            .await
            .is_ok()
        {
            if let Some(ref perms) = target_perms {
                let _ = provider.set_permissions(&tmp_vfs, perms).await;
            }
            if provider.rename(&tmp_vfs, &vfs_path).await.is_err() {
                let fallback = Cursor::new(content);
                provider.write_stream(&vfs_path, Box::new(fallback)).await?;
                if let Some(ref perms) = target_perms {
                    let _ = provider.set_permissions(&vfs_path, perms).await;
                }
                let _ = provider.delete(&tmp_vfs).await;
            } else if let Some(ref perms) = target_perms {
                let _ = provider.set_permissions(&vfs_path, perms).await;
            }
        } else {
            let fallback = Cursor::new(content);
            provider.write_stream(&vfs_path, Box::new(fallback)).await?;
            if let Some(ref perms) = target_perms {
                let _ = provider.set_permissions(&vfs_path, perms).await;
            }
        }

        let meta = provider.stat(&vfs_path).await?;

        // Audit log and real-time event
        record_audit_log(
            &state.db,
            Some(&user.id),
            "FILE_WRITE",
            Some(connection_id),
            Some(&vfs_path.path),
            "SUCCESS",
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
            "FILE_MKDIR",
            Some(connection_id),
            Some(&vfs_path.path),
            "SUCCESS",
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

    /// Delete multiple files/directories concurrently with bounded concurrency (8 workers)
    pub async fn delete_files(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        paths: Vec<String>,
    ) -> Result<(Vec<String>, Vec<(String, String)>), AppError> {
        check_permission(&state.db, user, connection_id, PermissionAction::Delete).await?;

        let provider = state.get_provider(connection_id).await.ok_or_else(|| {
            VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
        })?;

        let semaphore = Arc::new(Semaphore::new(8));
        let mut tasks = JoinSet::new();

        for p in paths {
            let provider = provider.clone();
            let conn_id = connection_id.to_string();
            let sem = semaphore.clone();
            tasks.spawn(async move {
                let _permit = sem.acquire().await;
                let vfs_path = match VfsPath::new(&conn_id, &p) {
                    Ok(v) => v,
                    Err(e) => return (p, Err(e)),
                };
                let res = provider.delete(&vfs_path).await;
                (p, res)
            });
        }

        let mut succeeded = Vec::new();
        let mut failed = Vec::new();

        while let Some(join_res) = tasks.join_next().await {
            if let Ok((path, del_res)) = join_res {
                match del_res {
                    Ok(_) => {
                        record_audit_log(
                            &state.db,
                            Some(&user.id),
                            "FILE_DELETE",
                            Some(connection_id),
                            Some(&path),
                            "SUCCESS",
                            None,
                            None,
                        )
                        .await;

                        state.transfer_manager.broadcast_event(WsEvent::FileChange {
                            connection_id: connection_id.to_string(),
                            path: path.clone(),
                            action: "delete".to_string(),
                        });

                        succeeded.push(path);
                    }
                    Err(e) => failed.push((path, e.to_string())),
                }
            }
        }

        succeeded.sort();
        failed.sort_by(|a, b| a.0.cmp(&b.0));

        Ok((succeeded, failed))
    }

    /// Delete a single file or directory
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
            "FILE_DELETE",
            Some(connection_id),
            Some(&vfs_path.path),
            "SUCCESS",
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
            "FILE_RENAME",
            Some(connection_id),
            Some(&from_vfs.path),
            "SUCCESS",
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
            "FILE_CHMOD",
            Some(connection_id),
            Some(&vfs_path.path),
            "SUCCESS",
            None,
            Some(&format!("Mode changed to: {:04o}", mode)),
        )
        .await;

        Ok(())
    }
}
