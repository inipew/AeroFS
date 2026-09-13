use crate::errors::AppError;
use crate::ports::sync::SyncControl;
use crate::sync::{SyncJob, SyncManager, SyncOperationRow, SyncStrategy};
use async_trait::async_trait;

/// Infrastructure adapter implementation for the existing durable SyncManager.
/// The manager remains the recovery/runtime owner; request-facing services only
/// observe it through the application-owned SyncControl contract.
#[async_trait]
impl SyncControl for SyncManager {
    async fn create_job(
        &self,
        user_id: &str,
        source_connection_id: &str,
        source_path: &str,
        destination_connection_id: &str,
        destination_path: &str,
        strategy: SyncStrategy,
    ) -> Result<SyncJob, AppError> {
        SyncManager::create_job(
            self,
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
        SyncManager::list_jobs(self).await.map_err(AppError::Internal)
    }

    async fn list_operations(&self, job_id: &str) -> Result<Vec<SyncOperationRow>, AppError> {
        SyncManager::list_operations(self, job_id)
            .await
            .map_err(AppError::Internal)
    }

    async fn resolve_conflict(
        &self,
        job_id: &str,
        op_id: &str,
        resolution: &str,
    ) -> Result<(), AppError> {
        SyncManager::resolve_conflict(self, job_id, op_id, resolution)
            .await
            .map_err(AppError::Internal)
    }
}
