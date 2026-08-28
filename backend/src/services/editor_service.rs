use crate::auth::AuthenticatedUser;
use crate::domain::FileMetadata;
use crate::errors::AppError;
use crate::services::file_service::FileService;
use crate::state::AppState;
use tokio::io::AsyncReadExt;

pub struct EditorService;

impl EditorService {
    pub async fn read_for_editing(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        path: &str,
    ) -> Result<(String, Option<String>), AppError> {
        let meta = FileService::stat_file(state, user, connection_id, path).await?;
        let max_size = state.config.limits.max_editable_size;
        if meta.size > max_size {
            return Err(AppError::PayloadTooLarge(format!(
                "File size ({} bytes) exceeds maximum editable size ({} bytes)",
                meta.size, max_size
            )));
        }

        let provider = state.registry.get(connection_id).await.ok_or_else(|| {
            AppError::NotFound(format!("Connection '{}' not found", connection_id))
        })?;

        let vfs_path = crate::domain::VfsPath::new(connection_id, path)?;
        let mut stream = provider.read_stream(&vfs_path).await?;
        let mut data = Vec::new();
        stream
            .read_to_end(&mut data)
            .await
            .map_err(|e| anyhow::anyhow!("Read error: {}", e))?;

        let content = String::from_utf8(data)
            .map_err(|_| AppError::BadRequest("File contains non-UTF8 binary data".into()))?;

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
