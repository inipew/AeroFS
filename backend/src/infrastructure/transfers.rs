use crate::auth::audit::record_audit_log;
use crate::auth::permissions::{check_permission, PermissionAction};
use crate::auth::UserInfo;
use crate::db::DbPool;
use crate::domain::Actor;
use crate::errors::AppError;
use crate::ports::transfer::{
    TransferCapabilities as PortTransferCapabilities, TransferControl, TransferEffects,
    TransferExecutionMode as PortTransferExecutionMode, TransferJob as PortTransferJob,
    TransferJobResponse as PortTransferJobResponse, TransferPhase as PortTransferPhase,
    TransferQueue, TransferStaging as PortTransferStaging, TransferStatus as PortTransferStatus,
    TransferSubmission, TransferType as PortTransferType,
};
use crate::services::UploadLockManager;
use crate::transfer::{
    CancelTransferError, RetryTransferError, TransferCommand, TransferEngine, TransferJob,
    TransferManager, TransferType,
};
use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::Arc;

fn to_engine_type(value: PortTransferType) -> TransferType {
    match value {
        PortTransferType::Copy => TransferType::Copy,
        PortTransferType::Move => TransferType::Move,
        PortTransferType::Upload => TransferType::Upload,
        PortTransferType::Sync => TransferType::Sync,
    }
}

fn to_port_type(value: TransferType) -> PortTransferType {
    match value {
        TransferType::Copy => PortTransferType::Copy,
        TransferType::Move => PortTransferType::Move,
        TransferType::Upload => PortTransferType::Upload,
        TransferType::Sync => PortTransferType::Sync,
    }
}

fn to_port_response(job: TransferJob) -> PortTransferJobResponse {
    use crate::transfer::{TransferExecutionMode, TransferPhase, TransferStaging, TransferStatus};

    let capabilities = job.capabilities();
    PortTransferJobResponse {
        capabilities: PortTransferCapabilities {
            can_cancel: capabilities.can_cancel,
            can_pause: capabilities.can_pause,
            can_resume: capabilities.can_resume,
            can_retry: capabilities.can_retry,
        },
        job: PortTransferJob {
            id: job.id,
            user_id: job.user_id,
            name: job.name,
            transfer_type: to_port_type(job.transfer_type),
            source_connection_id: job.source_connection_id,
            source_path: job.source_path,
            destination_connection_id: job.destination_connection_id,
            destination_path: job.destination_path,
            status: match job.status {
                TransferStatus::Queued => PortTransferStatus::Queued,
                TransferStatus::Running => PortTransferStatus::Running,
                TransferStatus::CancellationRequested => PortTransferStatus::CancellationRequested,
                TransferStatus::Cancelled => PortTransferStatus::Cancelled,
                TransferStatus::Interrupted => PortTransferStatus::Interrupted,
                TransferStatus::Completed => PortTransferStatus::Completed,
                TransferStatus::Failed => PortTransferStatus::Failed,
            },
            phase: match job.phase {
                TransferPhase::Preparing => PortTransferPhase::Preparing,
                TransferPhase::Transferring => PortTransferPhase::Transferring,
                TransferPhase::Finalizing => PortTransferPhase::Finalizing,
                TransferPhase::Verifying => PortTransferPhase::Verifying,
                TransferPhase::CleaningUp => PortTransferPhase::CleaningUp,
                TransferPhase::Completed => PortTransferPhase::Completed,
            },
            execution_mode: match job.execution_mode {
                TransferExecutionMode::Inline => PortTransferExecutionMode::Inline,
                TransferExecutionMode::Background => PortTransferExecutionMode::Background,
                TransferExecutionMode::Resumable => PortTransferExecutionMode::Resumable,
            },
            staging: match job.staging {
                TransferStaging::None => PortTransferStaging::None,
                TransferStaging::LocalTemp => PortTransferStaging::LocalTemp,
                TransferStaging::ProviderTemp => PortTransferStaging::ProviderTemp,
            },
            transferred_bytes: job.transferred_bytes,
            total_bytes: job.total_bytes,
            speed_bytes_per_sec: job.speed_bytes_per_sec,
            eta_seconds: job.eta_seconds,
            checksum: job.checksum,
            error_message: job.error_message,
            dismissed_at: job.dismissed_at,
            created_at: job.created_at,
            updated_at: job.updated_at,
        },
    }
}

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
                transfer_type: to_engine_type(submission.transfer_type),
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
    async fn submitted(&self, actor: &Actor, submission: &TransferSubmission, job_id: &str) {
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

#[derive(Clone)]
pub struct SqliteTransferControl {
    db: DbPool,
    manager: TransferManager,
    upload_locks: Arc<UploadLockManager>,
}

impl SqliteTransferControl {
    pub fn new(db: DbPool, manager: TransferManager, upload_locks: Arc<UploadLockManager>) -> Self {
        Self {
            db,
            manager,
            upload_locks,
        }
    }

    fn user(actor: &Actor) -> UserInfo {
        UserInfo {
            id: actor.id.clone(),
            username: actor.username.clone(),
            is_admin: actor.is_admin,
        }
    }

    fn visible(actor: &Actor, job: &TransferJob, allowed: &HashSet<String>) -> bool {
        actor.is_admin
            || job.user_id.as_deref() == Some(actor.id.as_str())
            || (allowed.contains(&job.source_connection_id)
                && allowed.contains(&job.destination_connection_id))
    }

    async fn verify_connection_enabled(&self, connection_id: &str) -> Result<(), AppError> {
        if connection_id == "local" {
            return Ok(());
        }
        let row: Option<(i64,)> = sqlx::query_as("SELECT enabled FROM connections WHERE id = ?")
            .bind(connection_id)
            .fetch_optional(&self.db)
            .await
            .map_err(|error| {
                AppError::Internal(anyhow::anyhow!(
                    "Database error checking connection status: {}",
                    error
                ))
            })?;
        match row {
            Some((enabled,)) if enabled != 0 => Ok(()),
            Some(_) => Err(AppError::BadRequest(format!(
                "Storage connection '{}' is disabled",
                connection_id
            ))),
            None => Err(AppError::NotFound(format!(
                "Storage connection '{}' not found",
                connection_id
            ))),
        }
    }
}

#[async_trait]
impl TransferControl for SqliteTransferControl {
    async fn list(&self, actor: &Actor) -> Result<Vec<PortTransferJobResponse>, AppError> {
        let mut jobs = self
            .manager
            .list_jobs(Some(&actor.id), actor.is_admin, false)
            .await;

        if !actor.is_admin {
            let rows: Vec<(String,)> = sqlx::query_as(
                "SELECT connection_id FROM permissions WHERE user_id = ? AND (can_read = 1 OR can_write = 1)",
            )
            .bind(&actor.id)
            .fetch_all(&self.db)
            .await
            .unwrap_or_default();
            let mut allowed: HashSet<String> = rows.into_iter().map(|row| row.0).collect();
            allowed.insert("local".to_string());
            jobs.retain(|job| Self::visible(actor, job, &allowed));
        }

        Ok(jobs.into_iter().map(to_port_response).collect())
    }

    async fn cancel(&self, actor: &Actor, job_id: &str) -> Result<(), AppError> {
        match self
            .manager
            .cancel_job(job_id, Some(&actor.id), actor.is_admin)
            .await
        {
            Ok(_) => {
                self.upload_locks.release(job_id).await;
                record_audit_log(
                    &self.db,
                    Some(&actor.id),
                    "TRANSFER_CANCEL",
                    None,
                    None,
                    "SUCCESS",
                    None,
                    Some(&format!("Cancelled transfer job {}", job_id)),
                )
                .await;
                Ok(())
            }
            Err(CancelTransferError::NotFound(id)) => Err(AppError::NotFound(format!(
                "Transfer job '{}' not found",
                id
            ))),
            Err(CancelTransferError::Unauthorized) => Err(AppError::Forbidden(
                "Permission denied: cannot cancel another user's transfer".into(),
            )),
            Err(CancelTransferError::NotCancellable(id)) => Err(AppError::Conflict(format!(
                "Transfer job '{}' cannot be cancelled in its current state",
                id
            ))),
            Err(CancelTransferError::Internal(error)) => {
                Err(AppError::Internal(anyhow::anyhow!(error)))
            }
        }
    }

    async fn retry(&self, actor: &Actor, job_id: &str) -> Result<(), AppError> {
        let job = self
            .manager
            .get_job(job_id)
            .await
            .ok_or_else(|| AppError::NotFound(format!("Transfer job '{}' not found", job_id)))?;

        if job.dismissed_at.is_some() {
            return Err(AppError::BadRequest(format!(
                "Transfer job '{}' has been dismissed and cannot be retried",
                job_id
            )));
        }
        if !actor.is_admin && job.user_id.as_deref() != Some(actor.id.as_str()) {
            return Err(AppError::Forbidden(
                "Permission denied: cannot retry another user's transfer".into(),
            ));
        }

        if !actor.is_admin {
            let user = Self::user(actor);
            check_permission(
                &self.db,
                &user,
                &job.source_connection_id,
                PermissionAction::Read,
            )
            .await?;
            if job.transfer_type == TransferType::Move {
                check_permission(
                    &self.db,
                    &user,
                    &job.source_connection_id,
                    PermissionAction::Delete,
                )
                .await?;
            }
            check_permission(
                &self.db,
                &user,
                &job.destination_connection_id,
                PermissionAction::Write,
            )
            .await?;
            check_permission(
                &self.db,
                &user,
                &job.destination_connection_id,
                PermissionAction::Create,
            )
            .await?;
        }

        self.verify_connection_enabled(&job.source_connection_id)
            .await?;
        self.verify_connection_enabled(&job.destination_connection_id)
            .await?;

        match self
            .manager
            .retry_job(job_id, Some(&actor.id), actor.is_admin)
            .await
        {
            Ok(_) => {
                record_audit_log(
                    &self.db,
                    Some(&actor.id),
                    "TRANSFER_RETRY",
                    None,
                    None,
                    "SUCCESS",
                    None,
                    Some(&format!("Retried transfer job {}", job_id)),
                )
                .await;
                Ok(())
            }
            Err(RetryTransferError::NotFound(id)) => Err(AppError::NotFound(format!(
                "Transfer job '{}' not found",
                id
            ))),
            Err(RetryTransferError::Unauthorized) => Err(AppError::Forbidden(
                "Permission denied: cannot retry another user's transfer".into(),
            )),
            Err(RetryTransferError::InvalidStatus(id, message)) => Err(AppError::BadRequest(
                format!("Cannot retry transfer '{}': {}", id, message),
            )),
            Err(RetryTransferError::SourceUnavailable(message))
            | Err(RetryTransferError::ProviderUnavailable(message)) => {
                Err(AppError::BadRequest(message))
            }
            Err(RetryTransferError::Dismissed(id)) => Err(AppError::BadRequest(format!(
                "Cannot retry transfer '{}': transfer has been dismissed",
                id
            ))),
            Err(RetryTransferError::Internal(error)) => {
                Err(AppError::Internal(anyhow::anyhow!(error)))
            }
        }
    }

    async fn dismiss(&self, actor: &Actor, job_id: &str) -> Result<(), AppError> {
        match self
            .manager
            .dismiss_job(job_id, Some(&actor.id), actor.is_admin)
            .await
        {
            Ok(true) => Ok(()),
            Ok(false) => Err(AppError::NotFound(format!(
                "Transfer job '{}' not found",
                job_id
            ))),
            Err(error) => Err(AppError::Forbidden(error)),
        }
    }

    async fn clear_finished(&self, actor: &Actor) -> Result<usize, AppError> {
        self.manager
            .clear_finished_jobs(Some(&actor.id), actor.is_admin)
            .await
            .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))
    }
}
