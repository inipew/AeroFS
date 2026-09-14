use super::manager::history::{SyncHistoryPage, SyncPageCursor};
use super::manager::{SyncManager as RuntimeSyncManager, SyncOperationRow};
use super::models::{SyncJob, SyncStrategy};
use crate::db::DbPool;
use crate::events::EventJournal;
use crate::runtime::{ResourceBudget, TaskSupervisor};
use crate::transfer::TransferManager;
use crate::vfs::FileSystem;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Bounded public facade for the sync runtime.
///
/// The underlying manager owns execution, recovery, and persistence details. Its legacy
/// unbounded history helpers intentionally remain behind the private `manager` module so
/// application/infrastructure callers can only consume durable history through keyset pages.
#[derive(Clone)]
pub struct SyncManager {
    inner: RuntimeSyncManager,
}

impl SyncManager {
    pub fn new(
        db: DbPool,
        transfer_manager: TransferManager,
        supervisor: TaskSupervisor,
        resource_budget: Arc<ResourceBudget>,
        event_journal: Arc<EventJournal>,
        providers: Arc<RwLock<HashMap<String, Arc<dyn FileSystem>>>>,
    ) -> Self {
        Self {
            inner: RuntimeSyncManager::new(
                db,
                transfer_manager,
                supervisor,
                resource_budget,
                event_journal,
                providers,
            ),
        }
    }

    pub fn supervisor(&self) -> &TaskSupervisor {
        self.inner.supervisor()
    }

    pub async fn create_job(
        &self,
        user_id: &str,
        source_connection_id: &str,
        source_path: &str,
        destination_connection_id: &str,
        destination_path: &str,
        strategy: SyncStrategy,
    ) -> anyhow::Result<SyncJob> {
        self.inner
            .create_job(
                user_id,
                source_connection_id,
                source_path,
                destination_connection_id,
                destination_path,
                strategy,
            )
            .await
    }

    pub async fn list_jobs_page(
        &self,
        cursor: Option<&SyncPageCursor>,
        limit: Option<usize>,
    ) -> anyhow::Result<SyncHistoryPage<SyncJob>> {
        self.inner.list_jobs_page(cursor, limit).await
    }

    pub async fn list_operations_page(
        &self,
        job_id: &str,
        cursor: Option<&SyncPageCursor>,
        limit: Option<usize>,
    ) -> anyhow::Result<SyncHistoryPage<SyncOperationRow>> {
        self.inner.list_operations_page(job_id, cursor, limit).await
    }

    pub async fn recover_interrupted_jobs(&self) -> anyhow::Result<()> {
        self.inner.recover_interrupted_jobs().await
    }

    pub async fn resolve_conflict(
        &self,
        job_id: &str,
        op_id: &str,
        resolution: &str,
    ) -> anyhow::Result<()> {
        self.inner.resolve_conflict(job_id, op_id, resolution).await
    }

    pub async fn notify_transfer_completed(
        &self,
        transfer_job_id: &str,
        success: bool,
    ) -> anyhow::Result<()> {
        self.inner
            .notify_transfer_completed(transfer_job_id, success)
            .await
    }
}
