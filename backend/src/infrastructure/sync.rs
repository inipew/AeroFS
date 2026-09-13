use crate::errors::AppError;
use crate::ports::sync::SyncControl;
use crate::sync::{SyncJob, SyncManager, SyncOperationRow, SyncStrategy};
use async_trait::async_trait;
use std::sync::Arc;

#[derive(Clone)]
pub struct ManagerSyncControl {
    manager: Arc<SyncManager>,
}

impl ManagerSyncControl {
    pub fn new(manager: Arc<SyncManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl SyncControl for ManagerSyncControl {
    async fn create_job(
        &self,
        user_id: &str,
        source_connection_id: &str,
        source_path: &str,
        destination_connection_id: &str,
        destination_path: &str,
        strategy: SyncStrategy,
    ) -> Result<SyncJob, AppError> {
        self.manager
            .create_job(
                user_id,
                source_connection_id,
                source_path,
                destination_connection_id,
                destination_path,
                strategy,
            )
            .await
            .map_err(AppError::Internal)
    }

    async fn list_jobs(&self) -> Result<Vec<SyncJob>, AppError> {
        self.manager.list_jobs().await.map_err(AppError::Internal)
    }

    async fn list_operations(&self, job_id: &str) -> Result<Vec<SyncOperationRow>, AppError> {
        self.manager
            .list_operations(job_id)
            .await
            .map_err(AppError::Internal)
    }

    async fn resolve_conflict(
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
