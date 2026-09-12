use crate::domain::{Actor, ConnectionId};
use crate::errors::AppError;
use crate::ports::authorization::{Authorization, FileAction};
use crate::sync::{SyncJob, SyncManager, SyncOperationRow, SyncStrategy};
use std::sync::Arc;

/// Narrow capability exposed to the HTTP sync adapter.
///
/// SyncManager remains the runtime engine used by durable subscribers and
/// recovery. This facade owns request-facing authorization and prevents the
/// HTTP layer from reaching persistence or runtime engine internals directly.
#[derive(Clone)]
pub struct SyncService {
    authorization: Arc<dyn Authorization>,
    manager: Arc<SyncManager>,
}

impl SyncService {
    pub fn new(authorization: Arc<dyn Authorization>, manager: Arc<SyncManager>) -> Self {
        Self {
            authorization,
            manager,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_job(
        &self,
        actor: &Actor,
        source_connection_id: &str,
        source_path: &str,
        destination_connection_id: &str,
        destination_path: &str,
        strategy: SyncStrategy,
    ) -> Result<SyncJob, AppError> {
        let source = ConnectionId(source_connection_id.to_string());
        let destination = ConnectionId(destination_connection_id.to_string());

        self.authorization
            .authorize(actor, &source, FileAction::Read)
            .await?;
        self.authorization
            .authorize(actor, &destination, FileAction::Write)
            .await?;

        self.manager
            .create_job(
                &actor.id,
                source_connection_id,
                source_path,
                destination_connection_id,
                destination_path,
                strategy,
            )
            .await
            .map_err(AppError::Internal)
    }

    pub async fn list_jobs(&self) -> Result<Vec<SyncJob>, AppError> {
        self.manager.list_jobs().await.map_err(AppError::Internal)
    }

    pub async fn list_operations(&self, job_id: &str) -> Result<Vec<SyncOperationRow>, AppError> {
        self.manager
            .list_operations(job_id)
            .await
            .map_err(AppError::Internal)
    }

    pub async fn resolve_conflict(
        &self,
        job_id: &str,
        op_id: &str,
        resolution: &str,
    ) -> Result<(), AppError> {
        self.manager
            .resolve_conflict(job_id, op_id, resolution)
            .await
            .map_err(AppError::Internal)
    }
}
