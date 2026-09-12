use crate::auth::AuthenticatedUser;
use crate::domain::{Actor, ConnectionId, FileMetadata};
use crate::errors::AppError;
use crate::state::AppState;

fn actor(user: &AuthenticatedUser) -> Actor {
    Actor {
        id: user.id.clone(),
        username: user.username.clone(),
        is_admin: user.is_admin,
    }
}

pub struct EditorService;

impl EditorService {
    pub async fn read_for_editing(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        path: &str,
    ) -> Result<(String, Option<String>), AppError> {
        let connection = ConnectionId::new(connection_id.to_string())
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        let meta = if let Some(metadata) = state
            .file_api
            .service
            .cached_metadata(connection_id, path)
            .await
        {
            metadata
        } else {
            let metadata = state
                .file_api
                .files
                .stat_file
                .execute(
                    &actor(user),
                    crate::application::files::StatFileCommand {
                        connection: connection.clone(),
                        path: path.to_string(),
                    },
                )
                .await?;
            state
                .file_api
                .service
                .cache_metadata(connection_id, path, metadata.clone())
                .await;
            metadata
        };
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
        let connection = ConnectionId::new(connection_id.to_string())
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        state
            .file_api
            .files
            .write_file
            .execute(
                &actor(user),
                crate::application::files::WriteFileCommand {
                    connection,
                    path: path.to_string(),
                    content: content.as_bytes().to_vec(),
                    expected_etag: expected_etag.map(str::to_string),
                    create_only: false,
                },
            )
            .await
    }
}
