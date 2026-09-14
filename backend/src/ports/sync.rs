use crate::errors::AppError;
use crate::sync::{
    SyncHistoryPage, SyncJob, SyncOperationRow, SyncPageCursor, SyncStrategy,
};
use async_trait::async_trait;

/// Request-facing sync control capability.
///
/// Durable replay and recovery continue to own the concrete SyncManager runtime;
/// application services depend only on this narrow control surface.
#[async_trait]
pub trait SyncControl: Send + Sync {
    async fn create_job(
        &self,
        user_id: &str,
        source_connection_id: &str,
        source_path: &str,
        destination_connection_id: &str,
        destination_path: &str,
        strategy: SyncStrategy,
    ) -> Result<SyncJob, AppError>;

    async fn list_jobs(
        &self,
        cursor: Option<&SyncPageCursor>,
        limit: Option<usize>,
    ) -> Result<SyncHistoryPage<SyncJob>, AppError>;

    async fn list_operations(
        &self,
        job_id: &str,
        cursor: Option<&SyncPageCursor>,
        limit: Option<usize>,
    ) -> Result<SyncHistoryPage<SyncOperationRow>, AppError>;

    async fn resolve_conflict(
        &self,
        job_id: &str,
        op_id: &str,
        resolution: &str,
    ) -> Result<(), AppError>;
}
