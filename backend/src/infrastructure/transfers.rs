use crate::auth::audit::record_audit_log;
use crate::db::DbPool;
use crate::domain::Actor;
use crate::errors::AppError;
use crate::ports::transfer::{TransferEffects, TransferQueue, TransferSubmission};
use crate::transfer::{TransferCommand, TransferEngine};
use async_trait::async_trait;

#[derive(Clone)]
pub struct TransferEngineQueue {
    engine: TransferEngine,
}

impl TransferEngineQueue {
    pub fn new(engine: TransferEngine) -> Self {
        Self { engine }
    }
}

#[async_trait]
impl TransferQueue for TransferEngineQueue {
    async fn submit(&self, submission: TransferSubmission) -> Result<String, AppError> {
        self.engine
            .submit(TransferCommand {
                user_id: submission.user_id,
                name: submission.name,
                transfer_type: submission.transfer_type,
                source_connection_id: submission.source_connection.to_string(),
                source_path: submission.source_path,
                destination_connection_id: submission.destination_connection.to_string(),
                destination_path: submission.destination_path,
            })
            .await
            .map(|admission| admission.job_id)
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
