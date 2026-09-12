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

pub struct PreviewService;

impl PreviewService {
    pub async fn get_preview_info(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        path: &str,
    ) -> Result<FileMetadata, AppError> {
        if let Some(metadata) = state
            .file_api
            .service
            .cached_metadata(connection_id, path)
            .await
        {
            return Ok(metadata);
        }

        let connection = ConnectionId::new(connection_id.to_string())
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        let metadata = state
            .file_api
            .files
            .stat_file
            .execute(
                &actor(user),
                crate::application::files::StatFileCommand {
                    connection,
                    path: path.to_string(),
                },
            )
            .await?;
        state
            .file_api
            .service
            .cache_metadata(connection_id, path, metadata.clone())
            .await;
        Ok(metadata)
    }
}
