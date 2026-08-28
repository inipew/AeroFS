use crate::auth::audit::record_audit_log;
use crate::auth::permissions::{check_permission, PermissionAction};
use crate::auth::AuthenticatedUser;
use crate::errors::AppError;
use crate::state::AppState;
use crate::transfer::{TransferJob, TransferType};
use std::collections::HashSet;

pub struct TransferService;

impl TransferService {
    /// Queue a new transfer job with full source (Read/Download, +Delete if Move) and destination (Write, Create) authorization
    #[allow(clippy::too_many_arguments)]
    pub async fn create_transfer(
        state: &AppState,
        user: &AuthenticatedUser,
        name: String,
        transfer_type: TransferType,
        source_connection_id: String,
        source_path: String,
        destination_connection_id: String,
        destination_path: String,
    ) -> Result<String, AppError> {
        // 1. Authorize source connection: Read / Download
        check_permission(
            &state.db,
            user,
            &source_connection_id,
            PermissionAction::Read,
        )
        .await?;

        // If Move transfer, user must also have Delete permission on source connection
        if transfer_type == TransferType::Move {
            check_permission(
                &state.db,
                user,
                &source_connection_id,
                PermissionAction::Delete,
            )
            .await?;
        }

        // 2. Authorize destination connection: Write / Create
        check_permission(
            &state.db,
            user,
            &destination_connection_id,
            PermissionAction::Write,
        )
        .await?;
        check_permission(
            &state.db,
            user,
            &destination_connection_id,
            PermissionAction::Create,
        )
        .await?;

        let job_id = state
            .transfer_manager
            .submit_job(
                Some(user.id.clone()),
                name,
                transfer_type,
                source_connection_id.clone(),
                source_path.clone(),
                destination_connection_id.clone(),
                destination_path.clone(),
            )
            .await
            .map_err(AppError::BadRequest)?;

        record_audit_log(
            &state.db,
            Some(&user.id),
            "TRANSFER_CREATE",
            Some(&source_connection_id),
            Some(&source_path),
            "SUCCESS",
            None,
            Some(&format!(
                "Job ID: {}, To: {}:{}",
                job_id, destination_connection_id, destination_path
            )),
        )
        .await;

        Ok(job_id)
    }

    /// List active and undismissed transfer jobs (scoped by user ownership and connection permissions)
    pub async fn list_transfers(
        state: &AppState,
        user: &AuthenticatedUser,
    ) -> Result<Vec<TransferJob>, AppError> {
        let mut jobs = state
            .transfer_manager
            .list_jobs(Some(&user.id), user.is_admin, false)
            .await;

        if !user.is_admin {
            let rows: Vec<(String,)> = sqlx::query_as(
                "SELECT connection_id FROM permissions WHERE user_id = ? AND can_read = 1",
            )
            .bind(&user.id)
            .fetch_all(&state.db)
            .await
            .unwrap_or_default();

            let mut allowed: HashSet<String> = rows.into_iter().map(|r| r.0).collect();
            allowed.insert("local".to_string());

            jobs.retain(|j| {
                allowed.contains(&j.source_connection_id)
                    && allowed.contains(&j.destination_connection_id)
            });
        }

        Ok(jobs)
    }

    /// Cancel an active transfer job (enforcing user ownership)
    pub async fn cancel_transfer(
        state: &AppState,
        user: &AuthenticatedUser,
        job_id: &str,
    ) -> Result<bool, AppError> {
        match state
            .transfer_manager
            .cancel_job(job_id, Some(&user.id), user.is_admin)
            .await
        {
            Ok(true) => {
                record_audit_log(
                    &state.db,
                    Some(&user.id),
                    "TRANSFER_CANCEL",
                    None,
                    None,
                    "SUCCESS",
                    None,
                    Some(&format!("Cancelled transfer job {}", job_id)),
                )
                .await;
                Ok(true)
            }
            Ok(false) => Err(AppError::NotFound(format!(
                "Transfer job '{}' not running or not found",
                job_id
            ))),
            Err(e) => Err(AppError::Forbidden(e)),
        }
    }

    /// Dismiss a single transfer job from history (persistent)
    pub async fn dismiss_transfer(
        state: &AppState,
        user: &AuthenticatedUser,
        job_id: &str,
    ) -> Result<bool, AppError> {
        match state
            .transfer_manager
            .dismiss_job(job_id, Some(&user.id), user.is_admin)
            .await
        {
            Ok(true) => Ok(true),
            Ok(false) => Err(AppError::NotFound(format!(
                "Transfer job '{}' not found",
                job_id
            ))),
            Err(e) => Err(AppError::Forbidden(e)),
        }
    }

    /// Dismiss all finished transfer jobs for the authenticated user (persistent Clear)
    pub async fn clear_finished_transfers(
        state: &AppState,
        user: &AuthenticatedUser,
    ) -> Result<usize, AppError> {
        match state
            .transfer_manager
            .clear_finished_jobs(Some(&user.id), user.is_admin)
            .await
        {
            Ok(cleared) => Ok(cleared),
            Err(e) => Err(AppError::Internal(anyhow::anyhow!(e))),
        }
    }
}
