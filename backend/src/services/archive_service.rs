use crate::auth::audit::record_audit_log;
use crate::auth::permissions::{check_permission, PermissionAction};
use crate::auth::AuthenticatedUser;
use crate::domain::VfsPath;
use crate::errors::{AppError, VfsError};
use crate::filesystem::archive::{
    compress_targz, compress_zip, extract_selected_archive_entries, extract_targz, extract_zip,
    list_virtual_archive_entries, read_virtual_archive_entry, ArchiveFormat, ArchiveOverwriteMode,
    VirtualArchiveEntry,
};
use crate::state::AppState;
use crate::transfer::WsEvent;

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct ArchiveResult {
    pub success: bool,
    pub message: String,
    pub entries_count: Option<usize>,
    pub skipped_count: Option<usize>,
}

pub struct ArchiveService;

impl ArchiveService {
    /// Compress files into a ZIP or TAR.GZ archive with authorization and audit logging
    pub async fn compress(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        base_path: &str,
        relative_paths: &[String],
        destination_file: &str,
        format_opt: Option<&str>,
    ) -> Result<ArchiveResult, AppError> {
        check_permission(&state.db, user, connection_id, PermissionAction::Create).await?;
        check_permission(&state.db, user, connection_id, PermissionAction::Write).await?;

        let _permit = state.archive_semaphore.acquire().await;

        let provider = state.get_provider(connection_id).await.ok_or_else(|| {
            VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
        })?;

        let dest_vfs = VfsPath::new(connection_id, destination_file)?;
        let format = format_opt
            .and_then(ArchiveFormat::from_path)
            .or_else(|| ArchiveFormat::from_path(destination_file))
            .unwrap_or(ArchiveFormat::Zip);

        match format {
            ArchiveFormat::Zip => {
                compress_zip(
                    &provider,
                    connection_id,
                    base_path,
                    relative_paths,
                    &dest_vfs,
                )
                .await?;
            }
            ArchiveFormat::TarGz => {
                compress_targz(
                    &provider,
                    connection_id,
                    base_path,
                    relative_paths,
                    &dest_vfs,
                )
                .await?;
            }
        }

        record_audit_log(
            &state.db,
            Some(&user.id),
            "ARCHIVE_COMPRESS",
            Some(connection_id),
            Some(&dest_vfs.path),
            "SUCCESS",
            None,
            Some(&format!("Created archive: {}", dest_vfs.path)),
        )
        .await;

        state.transfer_manager.broadcast_event(WsEvent::FileChange {
            connection_id: connection_id.to_string(),
            path: dest_vfs.path.clone(),
            action: "create".to_string(),
        });

        Ok(ArchiveResult {
            success: true,
            message: format!("Archive created: {}", dest_vfs.path),
            entries_count: Some(relative_paths.len()),
            skipped_count: None,
        })
    }

    /// Extract an archive into target directory with collision overwrite policy and dual-permissions (Create + Write)
    pub async fn extract(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        archive_path: &str,
        destination_dir: &str,
        format_opt: Option<&str>,
        overwrite_mode: ArchiveOverwriteMode,
    ) -> Result<ArchiveResult, AppError> {
        check_permission(&state.db, user, connection_id, PermissionAction::Create).await?;
        check_permission(&state.db, user, connection_id, PermissionAction::Write).await?;

        let _permit = state.archive_semaphore.acquire().await;

        let provider = state.get_provider(connection_id).await.ok_or_else(|| {
            VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
        })?;

        let archive_vfs = VfsPath::new(connection_id, archive_path)?;
        let format = format_opt
            .and_then(ArchiveFormat::from_path)
            .or_else(|| ArchiveFormat::from_path(archive_path))
            .unwrap_or(ArchiveFormat::Zip);

        let (count, skipped) = match format {
            ArchiveFormat::Zip => {
                extract_zip(&provider, &archive_vfs, destination_dir, overwrite_mode).await?
            }
            ArchiveFormat::TarGz => {
                extract_targz(&provider, &archive_vfs, destination_dir, overwrite_mode).await?
            }
        };

        record_audit_log(
            &state.db,
            Some(&user.id),
            "ARCHIVE_EXTRACT",
            Some(connection_id),
            Some(&archive_vfs.path),
            "SUCCESS",
            None,
            Some(&format!(
                "Extracted {} items (skipped {}) to {}",
                count, skipped, destination_dir
            )),
        )
        .await;

        state.transfer_manager.broadcast_event(WsEvent::FileChange {
            connection_id: connection_id.to_string(),
            path: destination_dir.to_string(),
            action: "extract".to_string(),
        });

        Ok(ArchiveResult {
            success: true,
            message: format!("Extracted {} item(s) to {}", count, destination_dir),
            entries_count: Some(count),
            skipped_count: Some(skipped),
        })
    }

    /// Extract only selected entries from an archive
    #[allow(clippy::too_many_arguments)]
    pub async fn extract_selected(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        archive_path: &str,
        destination_dir: &str,
        entries: &[String],
        _format_opt: Option<&str>,
        overwrite_mode: ArchiveOverwriteMode,
    ) -> Result<ArchiveResult, AppError> {
        check_permission(&state.db, user, connection_id, PermissionAction::Create).await?;
        check_permission(&state.db, user, connection_id, PermissionAction::Write).await?;

        let _permit = state.archive_semaphore.acquire().await;

        let provider = state.get_provider(connection_id).await.ok_or_else(|| {
            VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
        })?;

        let archive_vfs = VfsPath::new(connection_id, archive_path)?;

        let (count, skipped) = extract_selected_archive_entries(
            &provider,
            &archive_vfs,
            destination_dir,
            entries,
            overwrite_mode,
        )
        .await?;

        record_audit_log(
            &state.db,
            Some(&user.id),
            "ARCHIVE_EXTRACT_SELECTED",
            Some(connection_id),
            Some(&archive_vfs.path),
            "SUCCESS",
            None,
            Some(&format!(
                "Extracted {} selected items to {}",
                count, destination_dir
            )),
        )
        .await;

        state.transfer_manager.broadcast_event(WsEvent::FileChange {
            connection_id: connection_id.to_string(),
            path: destination_dir.to_string(),
            action: "extract".to_string(),
        });

        Ok(ArchiveResult {
            success: true,
            message: format!("Extracted {} item(s) to {}", count, destination_dir),
            entries_count: Some(count),
            skipped_count: Some(skipped),
        })
    }

    /// List virtual directory contents inside an archive without extracting
    pub async fn list_virtual(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        archive_path: &str,
        subpath: &str,
    ) -> Result<Vec<VirtualArchiveEntry>, AppError> {
        check_permission(&state.db, user, connection_id, PermissionAction::Read).await?;

        let provider = state.get_provider(connection_id).await.ok_or_else(|| {
            VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
        })?;

        let archive_vfs = VfsPath::new(connection_id, archive_path)?;
        let entries = list_virtual_archive_entries(&provider, &archive_vfs, subpath).await?;
        Ok(entries)
    }

    /// Read a single file entry from an archive directly into memory
    pub async fn read_virtual_entry(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        archive_path: &str,
        entry_path: &str,
    ) -> Result<(String, Vec<u8>), AppError> {
        check_permission(&state.db, user, connection_id, PermissionAction::Read).await?;

        let provider = state.get_provider(connection_id).await.ok_or_else(|| {
            VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
        })?;

        let archive_vfs = VfsPath::new(connection_id, archive_path)?;
        let (filename, bytes) =
            read_virtual_archive_entry(&provider, &archive_vfs, entry_path).await?;
        Ok((filename, bytes))
    }
}
