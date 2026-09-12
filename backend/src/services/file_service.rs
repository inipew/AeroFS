//! FileService — compatibility facade for older non-HTTP callers.
//! Delegates to application use-cases already composed in `AppState`.

use crate::auth::AuthenticatedUser;
use crate::domain::{Actor, ConnectionId, DirectoryListing, FileMetadata};
use crate::errors::AppError;
use crate::state::AppState;

fn actor(user: &AuthenticatedUser) -> Actor {
    Actor {
        id: user.id.clone(),
        username: user.username.clone(),
        is_admin: user.is_admin,
    }
}

pub struct FileService;

impl FileService {
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
        let connection = ConnectionId::new(connection_id.to_string())
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        let sort = sort_field_opt.map(|value| {
            value
                .parse::<crate::domain::SortField>()
                .unwrap_or(crate::domain::SortField::Name)
        });
        let order = sort_order_opt.map(|value| {
            if value.eq_ignore_ascii_case("desc") {
                crate::domain::SortOrder::Desc
            } else {
                crate::domain::SortOrder::Asc
            }
        });
        state
            .files
            .list_directory
            .execute(
                &actor(user),
                crate::application::files::ListDirectoryCommand {
                    connection,
                    path: raw_path,
                    show_hidden: show_hidden_opt,
                    sort,
                    order,
                    cursor: cursor_opt.map(str::to_string),
                    limit: limit_opt,
                },
            )
            .await
    }

    pub async fn get_presigned_download_url(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        raw_path: &str,
        expire_secs: Option<u64>,
    ) -> Result<String, AppError> {
        let connection = ConnectionId::new(connection_id.to_string())
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        state
            .files
            .presign_download
            .execute(
                &actor(user),
                crate::application::files::PresignCommand {
                    connection,
                    path: raw_path.to_string(),
                    expire_secs: expire_secs.unwrap_or(3600),
                },
            )
            .await
    }

    pub async fn get_presigned_upload_url(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        raw_path: &str,
        expire_secs: Option<u64>,
    ) -> Result<String, AppError> {
        let connection = ConnectionId::new(connection_id.to_string())
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        state
            .files
            .presign_upload
            .execute(
                &actor(user),
                crate::application::files::PresignCommand {
                    connection,
                    path: raw_path.to_string(),
                    expire_secs: expire_secs.unwrap_or(3600),
                },
            )
            .await
    }

    pub async fn complete_presigned_upload(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        raw_path: &str,
        expected_size: Option<u64>,
        expected_checksum: Option<&str>,
    ) -> Result<FileMetadata, AppError> {
        let connection = ConnectionId::new(connection_id.to_string())
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        state
            .files
            .complete_presigned
            .execute(
                &actor(user),
                crate::application::files::CompletePresignedCommand {
                    connection,
                    path: raw_path.to_string(),
                    expected_size,
                    expected_checksum: expected_checksum.map(str::to_string),
                },
            )
            .await
    }

    pub async fn stat_file(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        raw_path: &str,
    ) -> Result<FileMetadata, AppError> {
        let connection = ConnectionId::new(connection_id.to_string())
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        state
            .files
            .stat_file
            .execute(
                &actor(user),
                crate::application::files::StatFileCommand {
                    connection,
                    path: raw_path.to_string(),
                },
            )
            .await
    }

    pub async fn create_or_write_file(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        raw_path: &str,
        content: Vec<u8>,
        expected_etag: Option<&str>,
    ) -> Result<FileMetadata, AppError> {
        let connection = ConnectionId::new(connection_id.to_string())
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        state
            .files
            .write_file
            .execute(
                &actor(user),
                crate::application::files::WriteFileCommand {
                    connection,
                    path: raw_path.to_string(),
                    content,
                    expected_etag: expected_etag.map(str::to_string),
                    create_only: false,
                },
            )
            .await
    }

    pub async fn create_directory(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        raw_path: &str,
    ) -> Result<FileMetadata, AppError> {
        let connection = ConnectionId::new(connection_id.to_string())
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        state
            .files
            .create_directory
            .execute(
                &actor(user),
                crate::application::files::CreateDirectoryCommand {
                    connection,
                    path: raw_path.to_string(),
                },
            )
            .await
    }

    pub async fn delete_files(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        paths: Vec<String>,
    ) -> Result<(Vec<String>, Vec<(String, String)>), AppError> {
        let connection = ConnectionId::new(connection_id.to_string())
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        let result = state
            .files
            .delete_entries
            .execute(
                &actor(user),
                crate::application::files::DeleteEntriesCommand { connection, paths },
            )
            .await?;
        Ok((result.succeeded, result.failed))
    }

    pub async fn delete_entry(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        raw_path: &str,
    ) -> Result<(), AppError> {
        let (ok, fail) =
            Self::delete_files(state, user, connection_id, vec![raw_path.to_string()]).await?;
        if let Some((_, error)) = fail.first() {
            return Err(AppError::Internal(anyhow::anyhow!(error.clone())));
        }
        if ok.is_empty() {
            return Err(AppError::NotFound(format!("{} not found", raw_path)));
        }
        Ok(())
    }

    pub async fn rename_entry(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        from_raw: &str,
        to_raw: &str,
    ) -> Result<(), AppError> {
        let connection = ConnectionId::new(connection_id.to_string())
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        state
            .files
            .rename_entry
            .execute(
                &actor(user),
                crate::application::files::RenameEntryCommand {
                    connection,
                    from: from_raw.to_string(),
                    to: to_raw.to_string(),
                },
            )
            .await
    }

    pub async fn chmod(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        raw_path: &str,
        mode: u32,
    ) -> Result<(), AppError> {
        let connection = ConnectionId::new(connection_id.to_string())
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        state
            .files
            .chmod_entry
            .execute(
                &actor(user),
                crate::application::files::ChmodEntryCommand {
                    connection,
                    path: raw_path.to_string(),
                    mode,
                },
            )
            .await
    }
}
