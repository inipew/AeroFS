use crate::auth::audit::record_audit_log;
use crate::db::DbPool;
use crate::domain::Actor;
use crate::errors::AppError;
use crate::ports::transfer::{TransferEffects, TransferQueue, TransferSubmission};
use crate::transfer::TransferManager;
use async_trait::async_trait;

#[derive(Clone)]
pub struct TransferManagerQueue {
    manager: TransferManager,
}

impl TransferManagerQueue {
    pub fn new(manager: TransferManager) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl TransferQueue for TransferManagerQueue {
    async fn submit(&self, submission: TransferSubmission) -> Result<String, AppError> {
        self.manager
            .submit_job(
                submission.user_id,
                submission.name,
                submission.transfer_type,
                submission.source_connection.to_string(),
                submission.source_path,
                submission.destination_connection.to_string(),
                submission.destination_path,
            )
            .await
            .map_err(AppError::BadRequest)
    }
}

pub struct SqliteTransferEffects {
    db: DbPool,
}

impl SqliteTransferEffects {
    pub fn new(db: DbPool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl TransferEffects for SqliteTransferEffects {
    async fn submitted(
        &self,
        actor: &Actor,
        submission: &TransferSubmission,
        job_id: &str,
    ) {
        record_audit_log(
            &self.db,
            Some(&actor.id),
            "TRANSFER_CREATE",
            Some(submission.source_connection.as_str()),
            Some(&submission.source_path),
            "SUCCESS",
            None,
            Some(&format!(
                "Job ID: {}, To: {}:{}",
                job_id, submission.destination_connection, submission.destination_path
            )),
        )
        .await;
    }
}
