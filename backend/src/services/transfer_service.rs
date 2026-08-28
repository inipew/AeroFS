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

    /// Checks if a user has visibility authorization to view a specific transfer job
    pub fn authorize_transfer_visibility(
        user: &AuthenticatedUser,
        job: &TransferJob,
        allowed_connections: &HashSet<String>,
    ) -> bool {
        // 1. Admin always has full visibility
        if user.is_admin {
            return true;
        }

        // 2. Job Owner always has visibility to their own job
        if job.user_id.as_deref() == Some(&user.id) {
            return true;
        }

        // 3. For other jobs, user must have access to both source and destination connections
        allowed_connections.contains(&job.source_connection_id)
            && allowed_connections.contains(&job.destination_connection_id)
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
                "SELECT connection_id FROM permissions WHERE user_id = ? AND (can_read = 1 OR can_write = 1)",
            )
            .bind(&user.id)
            .fetch_all(&state.db)
            .await
            .unwrap_or_default();

            let mut allowed: HashSet<String> = rows.into_iter().map(|r| r.0).collect();
            allowed.insert("local".to_string());

            jobs.retain(|j| Self::authorize_transfer_visibility(user, j, &allowed));
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

    /// Retrieve a specific transfer job by ID directly from DB (for CLI / admin inspection)
    pub async fn get_transfer(
        pool: &crate::db::DbPool,
        job_id: &str,
    ) -> Result<Option<TransferJob>, AppError> {
        let row = sqlx::query(
            "SELECT id, user_id, name, transfer_type, source_connection_id, source_path,
                    destination_connection_id, destination_path, status, phase,
                    transferred_bytes, total_bytes, speed_bytes_per_sec, eta_seconds,
                    checksum, error_message, dismissed_at, created_at, updated_at
             FROM transfer_jobs WHERE id = ?",
        )
        .bind(job_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("DB query error: {}", e)))?;

        let r = match row {
            Some(r) => r,
            None => return Ok(None),
        };

        use chrono::DateTime;
        use sqlx::Row;

        let created_str: String = r.get("created_at");
        let updated_str: String = r.get("updated_at");
        let dismissed_str: Option<String> = r.get("dismissed_at");

        let created_at = DateTime::parse_from_rfc3339(&created_str)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now());
        let updated_at = DateTime::parse_from_rfc3339(&updated_str)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now());
        let dismissed_at = dismissed_str.and_then(|s| {
            DateTime::parse_from_rfc3339(&s)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .ok()
        });

        let job = TransferJob {
            id: r.get("id"),
            user_id: r.get("user_id"),
            name: r.get("name"),
            transfer_type: crate::transfer::TransferType::from_str(&r.get::<String, _>("transfer_type")),
            source_connection_id: r.get("source_connection_id"),
            source_path: r.get("source_path"),
            destination_connection_id: r.get("destination_connection_id"),
            destination_path: r.get("destination_path"),
            status: crate::transfer::TransferStatus::from_str(&r.get::<String, _>("status")),
            phase: crate::transfer::TransferPhase::from_str(&r.get::<String, _>("phase")),
            transferred_bytes: r.get::<i64, _>("transferred_bytes") as u64,
            total_bytes: r.get::<i64, _>("total_bytes") as u64,
            speed_bytes_per_sec: r.get::<i64, _>("speed_bytes_per_sec") as u64,
            eta_seconds: r.get::<Option<i64>, _>("eta_seconds").map(|v| v as u64),
            checksum: r.get("checksum"),
            error_message: r.get("error_message"),
            dismissed_at,
            created_at,
            updated_at,
        };

        Ok(Some(job))
    }

    /// List transfers with flexible filtering options for CLI administration
    pub async fn list_transfers_filtered(
        pool: &crate::db::DbPool,
        status: Option<&str>,
        limit: usize,
        user: Option<&str>,
        connection: Option<&str>,
    ) -> Result<Vec<TransferJob>, AppError> {
        use chrono::DateTime;
        use sqlx::Row;

        let mut query_str = "SELECT id, user_id, name, transfer_type, source_connection_id, source_path,
                                    destination_connection_id, destination_path, status, phase,
                                    transferred_bytes, total_bytes, speed_bytes_per_sec, eta_seconds,
                                    checksum, error_message, dismissed_at, created_at, updated_at
                             FROM transfer_jobs WHERE 1=1".to_string();

        if let Some(st) = status {
            let st_clean = st.trim().to_lowercase();
            if st_clean == "active" {
                query_str.push_str(" AND status IN ('queued', 'running')");
            } else if st_clean == "failed" {
                query_str.push_str(" AND status = 'failed'");
            } else if !st_clean.is_empty() {
                query_str.push_str(&format!(" AND status = '{}'", st_clean.replace('\'', "''")));
            }
        }

        if let Some(u) = user {
            query_str.push_str(&format!(" AND (user_id = '{0}' OR user_id IN (SELECT id FROM users WHERE username = '{0}'))", u.replace('\'', "''")));
        }

        if let Some(conn) = connection {
            query_str.push_str(&format!(" AND (source_connection_id = '{0}' OR destination_connection_id = '{0}')", conn.replace('\'', "''")));
        }

        query_str.push_str(&format!(" ORDER BY created_at DESC LIMIT {}", limit));

        let rows = sqlx::query(&query_str)
            .fetch_all(pool)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("DB query error: {}", e)))?;

        let mut jobs = Vec::new();
        for r in rows {
            let created_str: String = r.get("created_at");
            let updated_str: String = r.get("updated_at");
            let dismissed_str: Option<String> = r.get("dismissed_at");

            let created_at = DateTime::parse_from_rfc3339(&created_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());
            let updated_at = DateTime::parse_from_rfc3339(&updated_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());
            let dismissed_at = dismissed_str.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .ok()
            });

            jobs.push(TransferJob {
                id: r.get("id"),
                user_id: r.get("user_id"),
                name: r.get("name"),
                transfer_type: crate::transfer::TransferType::from_str(&r.get::<String, _>("transfer_type")),
                source_connection_id: r.get("source_connection_id"),
                source_path: r.get("source_path"),
                destination_connection_id: r.get("destination_connection_id"),
                destination_path: r.get("destination_path"),
                status: crate::transfer::TransferStatus::from_str(&r.get::<String, _>("status")),
                phase: crate::transfer::TransferPhase::from_str(&r.get::<String, _>("phase")),
                transferred_bytes: r.get::<i64, _>("transferred_bytes") as u64,
                total_bytes: r.get::<i64, _>("total_bytes") as u64,
                speed_bytes_per_sec: r.get::<i64, _>("speed_bytes_per_sec") as u64,
                eta_seconds: r.get::<Option<i64>, _>("eta_seconds").map(|v| v as u64),
                checksum: r.get("checksum"),
                error_message: r.get("error_message"),
                dismissed_at,
                created_at,
                updated_at,
            });
        }

        Ok(jobs)
    }

    /// Purge finished/dismissed transfers older than specified days with dry-run support
    pub async fn purge_transfers_older_than(
        pool: &crate::db::DbPool,
        days: u32,
        dry_run: bool,
    ) -> Result<usize, AppError> {
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(days as i64)).to_rfc3339();

        if dry_run {
            let count: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM transfer_jobs
                 WHERE (dismissed_at IS NOT NULL OR status IN ('completed', 'cancelled', 'failed'))
                 AND created_at < ?",
            )
            .bind(&cutoff)
            .fetch_one(pool)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("DB query error: {}", e)))?;

            Ok(count.0 as usize)
        } else {
            let res = sqlx::query(
                "DELETE FROM transfer_jobs
                 WHERE (dismissed_at IS NOT NULL OR status IN ('completed', 'cancelled', 'failed'))
                 AND created_at < ?",
            )
            .bind(&cutoff)
            .execute(pool)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("DB query error: {}", e)))?;

            Ok(res.rows_affected() as usize)
        }
    }

    /// Clean up stuck jobs (e.g. status='running' leftover from abrupt server termination)
    pub async fn repair_stuck_transfers(
        pool: &crate::db::DbPool,
        dry_run: bool,
    ) -> Result<usize, AppError> {
        if dry_run {
            let count: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM transfer_jobs WHERE status IN ('running', 'cancellation_requested')",
            )
            .fetch_one(pool)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("DB query error: {}", e)))?;

            Ok(count.0 as usize)
        } else {
            let now = chrono::Utc::now().to_rfc3339();
            let res = sqlx::query(
                "UPDATE transfer_jobs SET status = 'failed', error_message = 'Interrupted: Daemon terminated during active transfer', updated_at = ?
                 WHERE status IN ('running', 'cancellation_requested')",
            )
            .bind(&now)
            .execute(pool)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("DB query error: {}", e)))?;

            Ok(res.rows_affected() as usize)
        }
    }
}
