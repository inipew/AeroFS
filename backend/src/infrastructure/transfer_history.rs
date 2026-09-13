use crate::db::DbPool;
use crate::errors::AppError;
use crate::ports::transfer::{
    TransferExecutionMode, TransferHistoryFilter, TransferHistoryRepository, TransferJob,
    TransferPhase, TransferStaging, TransferStatus, TransferType,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{sqlite::SqliteRow, QueryBuilder, Row, Sqlite};

#[derive(Clone)]
pub struct SqliteTransferHistoryRepository {
    db: DbPool,
}

impl SqliteTransferHistoryRepository {
    pub fn new(db: DbPool) -> Self {
        Self { db }
    }
}

const TRANSFER_COLUMNS: &str =
    "id, user_id, name, transfer_type, source_connection_id, source_path, \
     destination_connection_id, destination_path, status, phase, execution_mode, staging, \
     transferred_bytes, total_bytes, speed_bytes_per_sec, eta_seconds, checksum, error_message, \
     dismissed_at, created_at, updated_at";

fn corrupted(field: &str, value: impl std::fmt::Display) -> AppError {
    AppError::Internal(anyhow::anyhow!(
        "invalid persisted transfer {field}: {value}"
    ))
}

fn parse_type(value: &str) -> Result<TransferType, AppError> {
    match value {
        "copy" => Ok(TransferType::Copy),
        "move" => Ok(TransferType::Move),
        "upload" => Ok(TransferType::Upload),
        "sync" => Ok(TransferType::Sync),
        other => Err(corrupted("type", other)),
    }
}

fn parse_status(value: &str) -> Result<TransferStatus, AppError> {
    match value {
        "queued" => Ok(TransferStatus::Queued),
        "running" => Ok(TransferStatus::Running),
        "cancellation_requested" => Ok(TransferStatus::CancellationRequested),
        "cancelled" => Ok(TransferStatus::Cancelled),
        "interrupted" => Ok(TransferStatus::Interrupted),
        "completed" => Ok(TransferStatus::Completed),
        "failed" => Ok(TransferStatus::Failed),
        other => Err(corrupted("status", other)),
    }
}

fn parse_phase(value: &str) -> Result<TransferPhase, AppError> {
    match value {
        "preparing" => Ok(TransferPhase::Preparing),
        "transferring" => Ok(TransferPhase::Transferring),
        "finalizing" => Ok(TransferPhase::Finalizing),
        "verifying" => Ok(TransferPhase::Verifying),
        "cleaning_up" => Ok(TransferPhase::CleaningUp),
        "completed" => Ok(TransferPhase::Completed),
        other => Err(corrupted("phase", other)),
    }
}

fn parse_execution_mode(value: &str) -> Result<TransferExecutionMode, AppError> {
    match value {
        "inline" => Ok(TransferExecutionMode::Inline),
        "background" => Ok(TransferExecutionMode::Background),
        "resumable" => Ok(TransferExecutionMode::Resumable),
        other => Err(corrupted("execution mode", other)),
    }
}

fn parse_staging(value: &str) -> Result<TransferStaging, AppError> {
    match value {
        "none" => Ok(TransferStaging::None),
        "local_temp" => Ok(TransferStaging::LocalTemp),
        "provider_temp" => Ok(TransferStaging::ProviderTemp),
        other => Err(corrupted("staging mode", other)),
    }
}

fn parse_timestamp(field: &str, value: &str) -> Result<DateTime<Utc>, AppError> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|error| corrupted(field, format!("{value:?}: {error}")))
}

fn nonnegative(field: &str, value: i64) -> Result<u64, AppError> {
    u64::try_from(value).map_err(|_| corrupted(field, value))
}

fn decode_job(row: SqliteRow) -> Result<TransferJob, AppError> {
    macro_rules! column {
        ($name:literal) => {
            row.try_get($name).map_err(|error| {
                AppError::Internal(anyhow::anyhow!(
                    "failed to decode persisted transfer column '{}': {}",
                    $name,
                    error
                ))
            })?
        };
    }

    let transfer_type: String = column!("transfer_type");
    let status: String = column!("status");
    let phase: String = column!("phase");
    let execution_mode: String = column!("execution_mode");
    let staging: String = column!("staging");
    let dismissed_at: Option<String> = column!("dismissed_at");
    let created_at: String = column!("created_at");
    let updated_at: String = column!("updated_at");
    let eta_seconds: Option<i64> = column!("eta_seconds");

    Ok(TransferJob {
        id: column!("id"),
        user_id: column!("user_id"),
        name: column!("name"),
        transfer_type: parse_type(&transfer_type)?,
        source_connection_id: column!("source_connection_id"),
        source_path: column!("source_path"),
        destination_connection_id: column!("destination_connection_id"),
        destination_path: column!("destination_path"),
        status: parse_status(&status)?,
        phase: parse_phase(&phase)?,
        execution_mode: parse_execution_mode(&execution_mode)?,
        staging: parse_staging(&staging)?,
        transferred_bytes: nonnegative("transferred_bytes", column!("transferred_bytes"))?,
        total_bytes: nonnegative("total_bytes", column!("total_bytes"))?,
        speed_bytes_per_sec: nonnegative("speed_bytes_per_sec", column!("speed_bytes_per_sec"))?,
        eta_seconds: eta_seconds
            .map(|value| nonnegative("eta_seconds", value))
            .transpose()?,
        checksum: column!("checksum"),
        error_message: column!("error_message"),
        dismissed_at: dismissed_at
            .as_deref()
            .map(|value| parse_timestamp("dismissed_at", value))
            .transpose()?,
        created_at: parse_timestamp("created_at", &created_at)?,
        updated_at: parse_timestamp("updated_at", &updated_at)?,
    })
}

#[async_trait]
impl TransferHistoryRepository for SqliteTransferHistoryRepository {
    async fn get(&self, job_id: &str) -> Result<Option<TransferJob>, AppError> {
        let sql = format!("SELECT {TRANSFER_COLUMNS} FROM transfer_jobs WHERE id = ?");
        let row = sqlx::query(&sql)
            .bind(job_id)
            .fetch_optional(&self.db)
            .await
            .map_err(|error| AppError::Internal(anyhow::anyhow!("DB query error: {error}")))?;
        row.map(decode_job).transpose()
    }

    async fn list(&self, filter: &TransferHistoryFilter) -> Result<Vec<TransferJob>, AppError> {
        let mut query: QueryBuilder<Sqlite> = QueryBuilder::new(format!(
            "SELECT {TRANSFER_COLUMNS} FROM transfer_jobs WHERE 1=1"
        ));

        if let Some(status) = filter.status.as_deref() {
            let status = status.trim().to_ascii_lowercase();
            match status.as_str() {
                "" => {}
                "active" => {
                    query.push(" AND status IN ('queued', 'running', 'cancellation_requested')");
                }
                other => {
                    query.push(" AND status = ").push_bind(other.to_string());
                }
            }
        }

        if let Some(user) = filter.user.as_deref() {
            query
                .push(" AND (user_id = ")
                .push_bind(user.to_string())
                .push(" OR user_id IN (SELECT id FROM users WHERE username = ")
                .push_bind(user.to_string())
                .push("))");
        }

        if let Some(connection) = filter.connection.as_deref() {
            query
                .push(" AND (source_connection_id = ")
                .push_bind(connection.to_string())
                .push(" OR destination_connection_id = ")
                .push_bind(connection.to_string())
                .push(")");
        }

        query
            .push(" ORDER BY created_at DESC LIMIT ")
            .push_bind(i64::try_from(filter.limit).unwrap_or(i64::MAX));

        let rows = query
            .build()
            .fetch_all(&self.db)
            .await
            .map_err(|error| AppError::Internal(anyhow::anyhow!("DB query error: {error}")))?;
        rows.into_iter().map(decode_job).collect()
    }

    async fn purge_older_than(
        &self,
        cutoff: DateTime<Utc>,
        dry_run: bool,
    ) -> Result<usize, AppError> {
        let cutoff = cutoff.to_rfc3339();
        let terminal = "(dismissed_at IS NOT NULL OR status IN ('completed', 'cancelled', 'failed', 'interrupted'))";
        if dry_run {
            let sql = format!("SELECT COUNT(*) FROM transfer_jobs WHERE {terminal} AND created_at < ?");
            let (count,): (i64,) = sqlx::query_as(&sql)
                .bind(&cutoff)
                .fetch_one(&self.db)
                .await
                .map_err(|error| AppError::Internal(anyhow::anyhow!("DB query error: {error}")))?;
            usize::try_from(count).map_err(|_| corrupted("purge count", count))
        } else {
            let sql = format!("DELETE FROM transfer_jobs WHERE {terminal} AND created_at < ?");
            let result = sqlx::query(&sql)
                .bind(&cutoff)
                .execute(&self.db)
                .await
                .map_err(|error| AppError::Internal(anyhow::anyhow!("DB query error: {error}")))?;
            Ok(result.rows_affected() as usize)
        }
    }

    async fn repair_stuck(&self, now: DateTime<Utc>, dry_run: bool) -> Result<usize, AppError> {
        if dry_run {
            let (count,): (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM transfer_jobs WHERE status IN ('running', 'cancellation_requested')",
            )
            .fetch_one(&self.db)
            .await
            .map_err(|error| AppError::Internal(anyhow::anyhow!("DB query error: {error}")))?;
            usize::try_from(count).map_err(|_| corrupted("repair count", count))
        } else {
            let result = sqlx::query(
                "UPDATE transfer_jobs
                 SET status = 'failed',
                     error_message = 'Interrupted: Daemon terminated during active transfer',
                     updated_at = ?
                 WHERE status IN ('running', 'cancellation_requested')",
            )
            .bind(now.to_rfc3339())
            .execute(&self.db)
            .await
            .map_err(|error| AppError::Internal(anyhow::anyhow!("DB query error: {error}")))?;
            Ok(result.rows_affected() as usize)
        }
    }
}
