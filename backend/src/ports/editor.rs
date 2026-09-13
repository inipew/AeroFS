use async_trait::async_trait;

use crate::domain::{Actor, ConnectionId, FileMetadata};
use crate::errors::AppError;

/// Narrow application-owned capability used by editor compatibility adapters.
///
/// This keeps editor orchestration independent from request/composition state while
/// allowing legacy callers to migrate incrementally.
#[async_trait]
pub trait EditorFileAccess: Send + Sync {
    async fn read_for_editing(
        &self,
        actor: &Actor,
        connection: &ConnectionId,
        path: &str,
    ) -> Result<(String, Option<String>), AppError>;

    async fn save_from_editing(
        &self,
        actor: &Actor,
        connection: ConnectionId,
        path: &str,
        content: Vec<u8>,
        expected_etag: Option<String>,
    ) -> Result<FileMetadata, AppError>;
}
