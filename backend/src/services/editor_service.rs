use crate::auth::AuthenticatedUser;
use crate::domain::{Actor, ConnectionId, FileMetadata};
use crate::errors::AppError;
use crate::ports::editor::EditorFileAccess;

fn actor(user: &AuthenticatedUser) -> Actor {
    Actor {
        id: user.id.clone(),
        username: user.username.clone(),
        is_admin: user.is_admin,
    }
}

/// Compatibility facade for editor-oriented callers.
///
/// The facade intentionally depends only on the application-owned `EditorFileAccess`
/// capability; it must not know request/composition state or concrete infrastructure.
pub struct EditorService;

impl EditorService {
    pub async fn read_for_editing<T: EditorFileAccess + ?Sized>(
        access: &T,
        user: &AuthenticatedUser,
        connection_id: &str,
        path: &str,
    ) -> Result<(String, Option<String>), AppError> {
        let connection = ConnectionId::new(connection_id.to_string())
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        access
            .read_for_editing(&actor(user), &connection, path)
            .await
    }

    pub async fn save_from_editing<T: EditorFileAccess + ?Sized>(
        access: &T,
        user: &AuthenticatedUser,
        connection_id: &str,
        path: &str,
        content: &str,
        expected_etag: Option<&str>,
    ) -> Result<FileMetadata, AppError> {
        let connection = ConnectionId::new(connection_id.to_string())
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        access
            .save_from_editing(
                &actor(user),
                connection,
                path,
                content.as_bytes().to_vec(),
                expected_etag.map(str::to_string),
            )
            .await
    }
}
