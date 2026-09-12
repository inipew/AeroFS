use crate::auth::AuthenticatedUser;
use crate::domain::{Actor, ConnectionId};
use crate::errors::AppError;
use crate::ports::transfer::{TransferJobResponse, TransferType};
use crate::state::AppState;
use crate::transfer::TransferJob;
use std::collections::HashSet;

pub struct TransferService;

impl TransferService {
    fn actor(user: &AuthenticatedUser) -> Actor {
        Actor {
            id: user.id.clone(),
            username: user.username.clone(),
            is_admin: user.is_admin,
        }
    }

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
        let source_connection = ConnectionId::new(source_connection_id)
            .map_err(|error| AppError::BadRequest(error.to_string()))?;
        let destination_connection = ConnectionId::new(destination_connection_id)
            .map_err(|error| AppError::BadRequest(error.to_string()))?;
        state
            .transfers
            .use_cases
            .create_transfer
            .execute(
                &Self::actor(user),
                crate::application::transfers::CreateTransferCommand {
                    name,
                    transfer_type,
                    source_connection,
                    source_path,
                    destination_connection,
                    destination_path,
                },
            )
            .await
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

    pub async fn list_transfers(
        state: &AppState,
        user: &AuthenticatedUser,
    ) -> Result<Vec<TransferJobResponse>, AppError> {
        state.transfers.use_cases.list(&Self::actor(user)).await
    }

    pub async fn cancel_transfer(
        state: &AppState,
        user: &AuthenticatedUser,
        job_id: &str,
    ) -> Result<bool, AppError> {
        state
            .transfers
            .use_cases
            .cancel(&Self::actor(user), job_id)
            .await?;
        Ok(true)
    }

    pub async fn retry_transfer(
        state: &AppState,
        user: &AuthenticatedUser,
        job_id: &str,
    ) -> Result<bool, AppError> {
        state
            .transfers
            .use_cases
            .retry(&Self::actor(user), job_id)
            .await?;
        Ok(true)
    }

    pub async fn dismiss_transfer(
        state: &AppState,
        user: &AuthenticatedUser,
        job_id: &str,
    ) -> Result<bool, AppError> {
        state
            .transfers
            .use_cases
            .dismiss(&Self::actor(user), job_id)
            .await?;
        Ok(true)
    }

    pub async fn clear_finished_transfers(
        state: &AppState,
        user: &AuthenticatedUser,
    ) -> Result<usize, AppError> {
        state
            .transfers
            .use_cases
            .clear_finished(&Self::actor(user))
            .await
    }

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

        Ok(Some(transfer_job_from_row(r)))
    }

    pub async fn list_transfers_filtered(
        pool: &crate::db::DbPool,
        status: Option<&str>,
        limit: usize,
        user: Option<&str>,
        connection: Option<&str>,
    ) -> Result<Vec<TransferJob>, AppError> {
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
            query_str.push_str(&format!(
                " AND (source_connection_id = '{0}' OR destination_connection_id = '{0}')",
                conn.replace('\'', "''")
            ));
        }
        query_str.push_str(&format!(" ORDER BY created_at DESC LIMIT {}", limit));

        let rows = sqlx::query(&query_str)
            .fetch_all(pool)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("DB query error: {}", e)))?;
        Ok(rows.into_iter().map(transfer_job_from_row).collect())
    }

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

fn transfer_job_from_row(r: sqlx::sqlite::SqliteRow) -> TransferJob {
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

    TransferJob {
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
        execution_mode: r
            .try_get::<String, _>("execution_mode")
            .ok()
            .map(|s| crate::transfer::TransferExecutionMode::from_str(&s))
            .unwrap_or_default(),
        staging: r
            .try_get::<String, _>("staging")
            .ok()
            .map(|s| crate::transfer::TransferStaging::from_str(&s))
            .unwrap_or_default(),
        transferred_bytes: r.get::<i64, _>("transferred_bytes") as u64,
        total_bytes: r.get::<i64, _>("total_bytes") as u64,
        speed_bytes_per_sec: r.get::<i64, _>("speed_bytes_per_sec") as u64,
        eta_seconds: r.get::<Option<i64>, _>("eta_seconds").map(|v| v as u64),
        checksum: r.get("checksum"),
        error_message: r.get("error_message"),
        dismissed_at,
        created_at,
        updated_at,
    }
}
