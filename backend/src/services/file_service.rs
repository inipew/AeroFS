//! FileService — thin facade deprecated (Phase 3.3). Delegates to FileApplicationService with explicit ports.
//! Keeps API for existing callers (tests, api/files legacy paths) but no god AppState logic inside.

use crate::auth::AuthenticatedUser;
use crate::domain::{ConnectionId, DirectoryListing, FileMetadata};
use crate::errors::AppError;
use crate::state::AppState;
use crate::application::FileApplicationService;

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
        Self::list_directory_paged(state, user, connection_id, raw_path, show_hidden_opt, sort_field_opt, sort_order_opt, None, None).await
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
        let conn = ConnectionId::new(connection_id.to_string()).map_err(|e| AppError::BadRequest(e.to_string()))?;
        let sort = sort_field_opt.map(|s| s.parse::<crate::domain::SortField>().unwrap_or(crate::domain::SortField::Name));
        let order = sort_order_opt.map(|s| if s.eq_ignore_ascii_case("desc") { crate::domain::SortOrder::Desc } else { crate::domain::SortOrder::Asc });
        let svc = FileApplicationService::from_state(state);
        svc.list_paged_owned(&user.0, &conn, crate::application::files::ListOptions {
            path: raw_path,
            show_hidden: show_hidden_opt,
            sort,
            order,
            cursor: cursor_opt.map(|s| s.to_string()),
            limit: limit_opt,
        }).await
    }

    pub async fn get_presigned_download_url(state: &AppState, user: &AuthenticatedUser, connection_id: &str, raw_path: &str, expire_secs: Option<u64>) -> Result<String, AppError> {
        let conn = ConnectionId::new(connection_id.to_string()).map_err(|e| AppError::BadRequest(e.to_string()))?;
        FileApplicationService::from_state(state).presign_download_typed(&user.0, &conn, raw_path.to_string(), expire_secs).await
    }

    pub async fn get_presigned_upload_url(state: &AppState, user: &AuthenticatedUser, connection_id: &str, raw_path: &str, expire_secs: Option<u64>) -> Result<String, AppError> {
        let conn = ConnectionId::new(connection_id.to_string()).map_err(|e| AppError::BadRequest(e.to_string()))?;
        FileApplicationService::from_state(state).presign_upload_typed(&user.0, &conn, raw_path.to_string(), expire_secs).await
    }

    pub async fn complete_presigned_upload(state: &AppState, user: &AuthenticatedUser, connection_id: &str, raw_path: &str, expected_size: Option<u64>, expected_checksum: Option<&str>) -> Result<FileMetadata, AppError> {
        let conn = ConnectionId::new(connection_id.to_string()).map_err(|e| AppError::BadRequest(e.to_string()))?;
        FileApplicationService::from_state(state).complete_presigned_typed(&user.0, &conn, raw_path.to_string(), expected_size, expected_checksum.map(|s| s.to_string())).await
    }

    pub async fn stat_file(state: &AppState, user: &AuthenticatedUser, connection_id: &str, raw_path: &str) -> Result<FileMetadata, AppError> {
        let conn = ConnectionId::new(connection_id.to_string()).map_err(|e| AppError::BadRequest(e.to_string()))?;
        FileApplicationService::from_state(state).stat_typed(&user.0, &conn, raw_path.to_string()).await
    }

    pub async fn create_or_write_file(state: &AppState, user: &AuthenticatedUser, connection_id: &str, raw_path: &str, content: Vec<u8>, expected_etag: Option<&str>) -> Result<FileMetadata, AppError> {
        let conn = ConnectionId::new(connection_id.to_string()).map_err(|e| AppError::BadRequest(e.to_string()))?;
        FileApplicationService::from_state(state).create_or_write_typed(&user.0, &conn, raw_path.to_string(), content, expected_etag.map(|s| s.to_string())).await
    }

    pub async fn create_directory(state: &AppState, user: &AuthenticatedUser, connection_id: &str, raw_path: &str) -> Result<FileMetadata, AppError> {
        let conn = ConnectionId::new(connection_id.to_string()).map_err(|e| AppError::BadRequest(e.to_string()))?;
        FileApplicationService::from_state(state).create_directory_typed(&user.0, &conn, raw_path.to_string()).await
    }

    pub async fn delete_files(state: &AppState, user: &AuthenticatedUser, connection_id: &str, paths: Vec<String>) -> Result<(Vec<String>, Vec<(String, String)>), AppError> {
        let conn = ConnectionId::new(connection_id.to_string()).map_err(|e| AppError::BadRequest(e.to_string()))?;
        FileApplicationService::from_state(state).delete_files_typed(&user.0, &conn, paths).await
    }

    pub async fn delete_entry(state: &AppState, user: &AuthenticatedUser, connection_id: &str, raw_path: &str) -> Result<(), AppError> {
        let (ok, fail) = Self::delete_files(state, user, connection_id, vec![raw_path.to_string()]).await?;
        if !fail.is_empty() { return Err(AppError::Internal(anyhow::anyhow!(fail[0].1.clone()))); }
        if ok.is_empty() { return Err(AppError::NotFound(format!("{} not found", raw_path))); }
        Ok(())
    }

    pub async fn rename_entry(state: &AppState, user: &AuthenticatedUser, connection_id: &str, from_raw: &str, to_raw: &str) -> Result<(), AppError> {
        let conn = ConnectionId::new(connection_id.to_string()).map_err(|e| AppError::BadRequest(e.to_string()))?;
        FileApplicationService::from_state(state).rename_typed(&user.0, &conn, from_raw.to_string(), to_raw.to_string()).await
    }

    pub async fn chmod(state: &AppState, user: &AuthenticatedUser, connection_id: &str, raw_path: &str, mode: u32) -> Result<(), AppError> {
        let conn = ConnectionId::new(connection_id.to_string()).map_err(|e| AppError::BadRequest(e.to_string()))?;
        FileApplicationService::from_state(state).chmod_typed(&user.0, &conn, raw_path.to_string(), mode).await
    }
}
