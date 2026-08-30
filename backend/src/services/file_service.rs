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
    /// List entries in a directory with backward-compatible non-paged signature
    pub async fn list_directory(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        raw_path: Option<String>,
        show_hidden_opt: Option<bool>,
        sort_field_opt: Option<&str>,
        sort_order_opt: Option<&str>,
    ) -> Result<DirectoryListing, AppError> {
        Self::list_directory_paged(
            state,
            user,
            connection_id,
            raw_path,
            show_hidden_opt,
            sort_field_opt,
            sort_order_opt,
            None,
            None,
        )
        .await
    }

    /// List entries in a directory using streaming lister with early filtering and opaque cursor pagination
    #[allow(clippy::too_many_arguments)]
    pub async fn list_directory_paged(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        raw_path: Option<String>,
        show_hidden_opt: Option<bool>,
        sort_field_opt: Option<&str>,
        sort_order_opt: Option<&str>,
        cursor_opt: Option<&str>,
        limit_opt: Option<usize>,
    ) -> Result<DirectoryListing, AppError> {
        use futures::StreamExt;

        check_permission(&state.db, user, connection_id, PermissionAction::Read).await?;

        let provider = state.get_provider(connection_id).await.ok_or_else(|| {
            VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
        })?;

        let path_str = raw_path.unwrap_or_else(|| "/".to_string());
        let vfs_path = VfsPath::new(connection_id, path_str)?;

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

        // Decode opaque cursor (offset based or last item marker)
        let skip_offset = if let Some(cursor_str) = cursor_opt {
            if let Ok(decoded) =
                base64::Engine::decode(&base64::engine::general_purpose::STANDARD, cursor_str)
            {
                if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&decoded) {
                    val["offset"].as_u64().unwrap_or(0) as usize
                } else {
                    String::from_utf8_lossy(&decoded)
                        .parse::<usize>()
                        .unwrap_or_default()
                }
            } else {
                cursor_str.parse::<usize>().unwrap_or(0)
            }
        } else {
            0
        };

        let page_limit = limit_opt
            .unwrap_or(state.config.limits.max_directory_entries)
            .clamp(1, 1000);
        let mut stream = provider.list_stream(&vfs_path).await?;
        let mut filtered_entries = Vec::new();
        let mut current_idx = 0;
        let mut has_more = false;

        while let Some(res) = stream.next().await {
            let entry = res?;

            // Early filter: skip internal staging files and hidden files if not requested
            if entry.name.contains(".aerofs-part-")
                || entry.name.contains(".aerofs.part")
                || entry.name.contains(".aerofs.tmp")
            {
                continue;
            }
            if !show_hidden && entry.is_hidden {
                continue;
            }

            if current_idx < skip_offset {
                current_idx += 1;
                continue;
            }

            if filtered_entries.len() < page_limit {
                filtered_entries.push(entry);
                current_idx += 1;
            } else {
                has_more = true;
                break;
            }
        }

        // Sort entries in the current page (directories first, then by field)
        let sort_field = sort_field_opt.unwrap_or("name");
        let sort_order = sort_order_opt.unwrap_or("asc");

        filtered_entries.sort_by(|a, b| {
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

        let next_cursor = if has_more {
            let cursor_payload = serde_json::json!({
                "offset": current_idx,
            });
            Some(base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                cursor_payload.to_string().as_bytes(),
            ))
        } else {
            None
        };

        let total_count = filtered_entries.len();

        Ok(DirectoryListing {
            path: vfs_path.path,
            connection_id: connection_id.to_string(),
            entries: filtered_entries,
            total_count: Some(total_count),
            has_more,
            next_cursor,
        })
    }

    /// Generate a pre-signed URL for direct browser download
    pub async fn get_presigned_download_url(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        raw_path: &str,
        expire_secs: Option<u64>,
    ) -> Result<String, AppError> {
        check_permission(&state.db, user, connection_id, PermissionAction::Read).await?;
        let provider = state.get_provider(connection_id).await.ok_or_else(|| {
            VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
        })?;
        let presign = provider.as_presign().ok_or_else(|| {
            VfsError::NotSupported(format!(
                "Pre-signed download URLs are not supported by provider '{}'",
                connection_id
            ))
        })?;
        let vfs_path = VfsPath::new(connection_id, raw_path)?;
        let ttl = std::time::Duration::from_secs(expire_secs.unwrap_or(3600).clamp(60, 86400));
        let url = presign.presign_read_url(&vfs_path, ttl).await?;
        record_audit_log(
            &state.db,
            Some(&user.id),
            "presign_download",
            Some(connection_id),
            Some(raw_path),
            "success",
            None,
            None,
        )
        .await;
        Ok(url)
    }

    /// Generate a pre-signed URL for direct browser upload
    pub async fn get_presigned_upload_url(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        raw_path: &str,
        expire_secs: Option<u64>,
    ) -> Result<String, AppError> {
        check_permission(&state.db, user, connection_id, PermissionAction::Write).await?;
        let provider = state.get_provider(connection_id).await.ok_or_else(|| {
            VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
        })?;
        let presign = provider.as_presign().ok_or_else(|| {
            VfsError::NotSupported(format!(
                "Pre-signed upload URLs are not supported by provider '{}'",
                connection_id
            ))
        })?;
        let vfs_path = VfsPath::new(connection_id, raw_path)?;
        let ttl = std::time::Duration::from_secs(expire_secs.unwrap_or(3600).clamp(60, 86400));
        let url = presign.presign_write_url(&vfs_path, ttl).await?;
        record_audit_log(
            &state.db,
            Some(&user.id),
            "presign_upload",
            Some(connection_id),
            Some(raw_path),
            "success",
            None,
            None,
        )
        .await;
        Ok(url)
    }

    /// Complete and verify a client-side direct pre-signed upload with validation and cache invalidation
    pub async fn complete_presigned_upload(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        raw_path: &str,
        expected_size: Option<u64>,
        expected_checksum: Option<&str>,
    ) -> Result<FileMetadata, AppError> {
        check_permission(&state.db, user, connection_id, PermissionAction::Write).await?;
        let provider = state.get_provider(connection_id).await.ok_or_else(|| {
            VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
        })?;
        let vfs_path = VfsPath::new(connection_id, raw_path)?;
        let meta = provider.stat(&vfs_path).await.map_err(|e| match e {
            VfsError::NotFound(_) => AppError::NotFound(format!(
                "Uploaded file not found at target path '{}'",
                vfs_path.path
            )),
            other => AppError::from(other),
        })?;

        // 1. Validate actual uploaded size if expected size was specified
        if let Some(exp_size) = expected_size {
            if meta.size != exp_size {
                return Err(AppError::BadRequest(format!(
                    "Uploaded file size mismatch: expected {} bytes, found {} bytes",
                    exp_size, meta.size
                )));
            }
        }

        // 2. Validate checksum if expected checksum was specified
        if let Some(exp_chk) = expected_checksum {
            let clean_exp = exp_chk.trim().trim_matches('"');
            let clean_etag = meta.etag.trim().trim_matches('"');
            if !clean_etag.is_empty() && !clean_etag.eq_ignore_ascii_case(clean_exp) {
                return Err(AppError::BadRequest(format!(
                    "Uploaded file checksum mismatch: expected '{}', found '{}'",
                    clean_exp, clean_etag
                )));
            }
        }

        // 3. Invalidate short-TTL metadata cache
        state
            .metadata_cache
            .invalidate(connection_id, raw_path)
            .await;

        record_audit_log(
            &state.db,
            Some(&user.id),
            "presign_upload_complete",
            Some(connection_id),
            Some(raw_path),
            "success",
            None,
            Some(&format!("size={}, etag={:?}", meta.size, meta.etag)),
        )
        .await;

        state
            .transfer_manager
            .broadcast_event(crate::transfer::WsEvent::file_change(
                connection_id,
                &vfs_path.path,
                "upload",
            ))
            .await;

        Ok(meta)
    }

    /// Retrieve file metadata with short-TTL cache and Single-Flight stampede protection
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

        state
            .metadata_cache
            .get_or_fetch(connection_id, raw_path, || async {
                let meta = provider.stat(&vfs_path).await?;
                Ok(meta)
            })
            .await
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

        // Optimistic concurrency check (ETag / If-Match)
        if let Some(expected) = expected_etag {
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

        // Destination permission preservation: capture existing permissions or inherit from parent
        let target_perms = resolve_destination_permissions(
            &provider,
            &vfs_path,
            false,
            PermissionInheritanceMode::InheritExistingOrParent,
        )
        .await;

        if caps.atomic_rename {
            // Atomic write safety: write to temporary path first then atomic rename
            let tmp_vfs = VfsPath::new(connection_id, format!("{}.aerofs.tmp", vfs_path.path))?;
            let cursor = Cursor::new(content.clone());

            if provider
                .write_stream(&tmp_vfs, Box::new(cursor))
                .await
                .is_ok()
            {
                if caps.permissions {
                    if let Some(ref perms) = target_perms {
                        if let Err(e) = provider.set_permissions(&tmp_vfs, perms).await {
                            tracing::warn!(
                                "Failed to set permissions on temporary file '{}': {}",
                                tmp_vfs.path,
                                e
                            );
                        }
                    }
                }

                if let Err(rename_err) = provider.rename(&tmp_vfs, &vfs_path).await {
                    tracing::warn!(
                        "Atomic rename failed from '{}' to '{}': {}. Falling back to direct write.",
                        tmp_vfs.path,
                        vfs_path.path,
                        rename_err
                    );
                    let _ = provider.delete(&tmp_vfs).await;

                    let fallback = Cursor::new(content);
                    provider.write_stream(&vfs_path, Box::new(fallback)).await?;
                    if caps.permissions {
                        if let Some(ref perms) = target_perms {
                            if let Err(e) = provider.set_permissions(&vfs_path, perms).await {
                                tracing::warn!(
                                    "Failed to set permissions on '{}': {}",
                                    vfs_path.path,
                                    e
                                );
                            }
                        }
                    }
                } else if caps.permissions {
                    if let Some(ref perms) = target_perms {
                        if let Err(e) = provider.set_permissions(&vfs_path, perms).await {
                            tracing::warn!(
                                "Failed to set permissions on '{}': {}",
                                vfs_path.path,
                                e
                            );
                        }
                    }
                }
            } else {
                let fallback = Cursor::new(content);
                provider.write_stream(&vfs_path, Box::new(fallback)).await?;
                if caps.permissions {
                    if let Some(ref perms) = target_perms {
                        if let Err(e) = provider.set_permissions(&vfs_path, perms).await {
                            tracing::warn!(
                                "Failed to set permissions on '{}': {}",
                                vfs_path.path,
                                e
                            );
                        }
                    }
                }
            }
        } else {
            // Direct write for providers without atomic rename support (e.g. object storage)
            let cursor = Cursor::new(content);
            provider.write_stream(&vfs_path, Box::new(cursor)).await?;
            if caps.permissions {
                if let Some(ref perms) = target_perms {
                    if let Err(e) = provider.set_permissions(&vfs_path, perms).await {
                        tracing::warn!("Failed to set permissions on '{}': {}", vfs_path.path, e);
                    }
                }
            }
        }

        let meta = provider.stat(&vfs_path).await?;

        // Invalidate short-TTL metadata cache on write
        state
            .metadata_cache
            .invalidate(connection_id, raw_path)
            .await;

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

        state.transfer_manager.broadcast_event(WsEvent::file_change(
            connection_id,
            &vfs_path.path,
            "write",
        )).await;

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

        // Invalidate short-TTL metadata cache on mkdir
        state
            .metadata_cache
            .invalidate(connection_id, raw_path)
            .await;

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

        state.transfer_manager.broadcast_event(WsEvent::file_change(
            connection_id,
            &vfs_path.path,
            "create",
        )).await;

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

        let mut succeeded = Vec::new();
        let mut failed = Vec::new();

        let mut tasks = JoinSet::new();
        let sem = Arc::new(Semaphore::new(8));

        for raw_path in paths {
            let p_clone = provider.clone();
            let conn_str = connection_id.to_string();
            let sem_clone = sem.clone();

            tasks.spawn(async move {
                let _permit = match sem_clone.acquire().await {
                    Ok(p) => p,
                    Err(_) => {
                        return (
                            raw_path,
                            Err(crate::errors::VfsError::IoError("Semaphore closed during shutdown".to_string())),
                        )
                    }
                };
                let vfs_path = match VfsPath::new(&conn_str, &raw_path) {
                    Ok(v) => v,
                    Err(e) => return (raw_path, Err(e)),
                };
                let res = p_clone.delete(&vfs_path).await;
                (raw_path, res)
            });
        }

        while let Some(join_res) = tasks.join_next().await {
            if let Ok((path, del_res)) = join_res {
                match del_res {
                    Ok(_) => {
                        // Invalidate short-TTL metadata cache on delete
                        state
                            .metadata_cache
                            .invalidate_prefix(connection_id, &path)
                            .await;

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

                        state.transfer_manager.broadcast_event(WsEvent::file_change(
                            connection_id,
                            &path,
                            "delete",
                        )).await;

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

        // Invalidate short-TTL metadata cache
        state
            .metadata_cache
            .invalidate_prefix(connection_id, raw_path)
            .await;

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

        state.transfer_manager.broadcast_event(WsEvent::file_change(
            connection_id,
            &vfs_path.path,
            "delete",
        )).await;

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

        // Invalidate both source and destination in metadata cache
        state
            .metadata_cache
            .invalidate_prefix(connection_id, from_raw)
            .await;
        state
            .metadata_cache
            .invalidate_prefix(connection_id, to_raw)
            .await;

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

        state.transfer_manager.broadcast_event(WsEvent::file_rename(
            connection_id,
            &from_vfs.path,
            &to_vfs.path,
        )).await;

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

        // Invalidate metadata cache on chmod
        state
            .metadata_cache
            .invalidate(connection_id, raw_path)
            .await;

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

        // Broadcast FileChange event for real-time inspector / panel refresh (Plan 58)
        state.transfer_manager.broadcast_event(WsEvent::file_change(
            connection_id,
            &vfs_path.path,
            "chmod",
        )).await;

        Ok(())
    }
}
