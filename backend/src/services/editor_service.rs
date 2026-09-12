use crate::auth::AuthenticatedUser;
use crate::domain::{Actor, ConnectionId, FileMetadata};
use crate::errors::AppError;
use crate::services::file_service::FileService;
use crate::state::AppState;

pub struct EditorService;

impl EditorService {
    pub async fn read_for_editing(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        path: &str,
    ) -> Result<(String, Option<String>), AppError> {
        let meta = FileService::stat_file(state, user, connection_id, path).await?;
        let connection = ConnectionId::new(connection_id.to_string())
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        let content = state
            .file_api
            .service
            .read_text_for_editing(&connection, path, meta.size)
            .await?;
        Ok((content, Some(meta.etag)))
    }

    pub async fn save_from_editing(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        path: &str,
        content: &str,
        expected_etag: Option<&str>,
    ) -> Result<FileMetadata, AppError> {
        let _actor = Actor {
            id: user.id.clone(),
            username: user.username.clone(),
            is_admin: user.is_admin,
        };
        FileService::create_or_write_file(
            state,
            user,
            connection_id,
            path,
            content.as_bytes().to_vec(),
            expected_etag,
        )
        .await
    }
}
