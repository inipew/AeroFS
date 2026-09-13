use crate::auth::AuthenticatedUser;
use crate::errors::AppError;
use crate::ports::transfer::{TransferHistoryFilter, TransferHistoryRepository, TransferJob};
use chrono::{Duration, Utc};
use std::collections::HashSet;
use std::sync::Arc;

/// Transfer history/maintenance capability for CLI and diagnostic adapters.
/// Request-facing transfer commands remain owned by `TransferUseCases`.
#[derive(Clone)]
pub struct TransferService {
    history: Arc<dyn TransferHistoryRepository>,
}

impl TransferService {
    pub fn new(history: Arc<dyn TransferHistoryRepository>) -> Self {
        Self { history }
    }

    pub fn authorize_transfer_visibility(
        user: &AuthenticatedUser,
        job: &TransferJob,
        allowed_connections: &HashSet<String>,
    ) -> bool {
        user.is_admin
            || job.user_id.as_deref() == Some(&user.id)
            || (allowed_connections.contains(&job.source_connection_id)
                && allowed_connections.contains(&job.destination_connection_id))
    }

    pub async fn get_transfer(&self, job_id: &str) -> Result<Option<TransferJob>, AppError> {
        self.history.get(job_id).await
    }

    pub async fn list_transfers_filtered(
        &self,
        status: Option<&str>,
        limit: usize,
        user: Option<&str>,
        connection: Option<&str>,
    ) -> Result<Vec<TransferJob>, AppError> {
        self.history
            .list(&TransferHistoryFilter {
                status: status.map(str::to_owned),
                limit,
                user: user.map(str::to_owned),
                connection: connection.map(str::to_owned),
            })
            .await
    }

    pub async fn purge_transfers_older_than(
        &self,
        days: u32,
        dry_run: bool,
    ) -> Result<usize, AppError> {
        let cutoff = Utc::now() - Duration::days(i64::from(days));
        self.history.purge_older_than(cutoff, dry_run).await
    }

    pub async fn repair_stuck_transfers(&self, dry_run: bool) -> Result<usize, AppError> {
        self.history.repair_stuck(Utc::now(), dry_run).await
    }
}
