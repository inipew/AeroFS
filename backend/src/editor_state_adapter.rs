use async_trait::async_trait;

use crate::application::files::{StatFileCommand, WriteFileCommand};
use crate::domain::{Actor, ConnectionId, FileMetadata};
use crate::errors::AppError;
use crate::ports::editor::EditorFileAccess;
use crate::state::FileApiState;

/// Adapter from request-facing file capability composition to the narrow editor port.
/// Keeping this outside `services` prevents editor orchestration from depending on state.
#[async_trait]
impl EditorFileAccess for FileApiState {
    async fn read_for_editing(
        &self,
        actor: &Actor,
        connection: &ConnectionId,
        path: &str,
    ) -> Result<(String, Option<String>), AppError> {
        let metadata = if let Some(metadata) = self.service.cached_metadata(connection.as_str(), path).await {
            metadata
        } else {
            let metadata = self
                .files
                .stat_file
                .execute(
                    actor,
                    StatFileCommand {
                        connection: connection.clone(),
                        path: path.to_string(),
                    },
                )
                .await?;
            self.service
                .cache_metadata(connection.as_str(), path, metadata.clone())
                .await;
            metadata
        };

        let content = self
            .service
            .read_text_for_editing(connection, path, metadata.size)
            .await?;
        Ok((content, Some(metadata.etag)))
    }

    async fn save_from_editing(
        &self,
        actor: &Actor,
        connection: ConnectionId,
        path: &str,
        content: Vec<u8>,
        expected_etag: Option<String>,
    ) -> Result<FileMetadata, AppError> {
        self.files
            .write_file
            .execute(
                actor,
                WriteFileCommand {
                    connection,
                    path: path.to_string(),
                    content,
                    expected_etag,
                    create_only: false,
                },
            )
            .await
    }
}
