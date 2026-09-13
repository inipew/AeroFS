use crate::domain::{Actor, ConnectionId};
use crate::errors::AppError;
use crate::ports::{
    authorization::{Authorization, FileAction},
    sync::SyncControl,
};
use crate::sync::{SyncJob, SyncOperationRow, SyncStrategy};
use std::sync::Arc;

/// Narrow capability exposed to the HTTP sync adapter.
///
/// The concrete runtime remains owned by durable replay/recovery infrastructure.
/// Request-facing orchestration depends only on SyncControl.
#[derive(Clone)]
pub struct SyncService {
    authorization: Arc<dyn Authorization>,
    control: Arc<dyn SyncControl>,
}

impl SyncService {
    pub fn new(authorization: Arc<dyn Authorization>, control: Arc<dyn SyncControl>) -> Self {
        Self {
            authorization,
            control,
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

        self.control
            .create_job(
                &actor.id,
                source_connection_id,
                source_path,
                destination_connection_id,
                destination_path,
                strategy,
            )
            .await
    }

    pub async fn list_jobs(&self) -> Result<Vec<SyncJob>, AppError> {
        self.control.list_jobs().await
    }

    pub async fn list_operations(&self, job_id: &str) -> Result<Vec<SyncOperationRow>, AppError> {
        self.control.list_operations(job_id).await
    }

    pub async fn resolve_conflict(
        &self,
        job_id: &str,
        op_id: &str,
        resolution: &str,
    ) -> Result<(), AppError> {
        self.control
            .resolve_conflict(job_id, op_id, resolution)
            .await
    }
}
