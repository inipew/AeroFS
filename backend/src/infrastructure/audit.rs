use crate::auth::audit::AuditLogEntry;
use crate::db::DbPool;
use crate::domain::settings::UserPreferences;
use crate::errors::AppError;
use crate::ports::audit::{AuditRecord, AuditRepository, UserPreferencesRepository};
use async_trait::async_trait;
use chrono::Utc;
use uuid::Uuid;

#[derive(Clone)]
pub struct SqliteAuditRepository {
    db: DbPool,
}

impl SqliteAuditRepository {
    pub fn new(db: DbPool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl AuditRepository for SqliteAuditRepository {
    async fn record(&self, record: AuditRecord<'_>) -> Result<(), AppError> {
        let id = format!("log_{}", &Uuid::new_v4().to_string()[..12]);
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO audit_logs (id, user_id, action, connection_id, path, status, ip_address, details, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(record.user_id)
        .bind(record.action)
        .bind(record.connection_id)
        .bind(record.resource_path)
        .bind(record.status)
        .bind(record.ip_address)
        .bind(record.details)
        .bind(now)
        .execute(&self.db)
        .await
        .map_err(|error| AppError::Internal(anyhow::anyhow!("failed to persist audit log: {error}")))?;
        Ok(())
    }

    async fn list(&self, limit: usize, offset: usize) -> Result<Vec<AuditLogEntry>, AppError> {
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let offset = i64::try_from(offset).unwrap_or(i64::MAX);
        let rows = sqlx::query_as::<_, (String, Option<String>, Option<String>, String, Option<String>, Option<String>, String, Option<String>, Option<String>, String)>(
            "SELECT a.id, a.user_id, u.username, a.action, a.connection_id, a.path, a.status, a.ip_address, a.details, a.created_at FROM audit_logs a LEFT JOIN users u ON a.user_id = u.id ORDER BY a.created_at DESC LIMIT ? OFFSET ?",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.db)
        .await
        .map_err(|error| AppError::Internal(anyhow::anyhow!("Database error: {error}")))?;

        Ok(rows.into_iter().map(|(id, user_id, username, action, connection_id, path, status, ip_address, details, created_at)| AuditLogEntry {
            id,
            user_id,
            username,
            action,
            connection_id,
            path,
            status,
            ip_address,
            details,
            created_at,
        }).collect())
    }
}

#[derive(Clone)]
pub struct SqliteUserPreferencesRepository {
    db: DbPool,
}

impl SqliteUserPreferencesRepository {
    pub fn new(db: DbPool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl UserPreferencesRepository for SqliteUserPreferencesRepository {
    async fn get(&self, user_id: &str) -> Result<Option<UserPreferences>, AppError> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT preferences_json FROM user_preferences WHERE user_id = ?",
        )
        .bind(user_id)
        .fetch_optional(&self.db)
        .await
        .map_err(|error| AppError::Internal(anyhow::anyhow!("DB error: {error}")))?;

        row.map(|(json,)| {
            serde_json::from_str(&json).map_err(|error| {
                AppError::Internal(anyhow::anyhow!(
                    "invalid persisted user preferences for {user_id}: {error}"
                ))
            })
        })
        .transpose()
    }

    async fn set(&self, user_id: &str, prefs: &UserPreferences) -> Result<(), AppError> {
        let json = serde_json::to_string(prefs).map_err(|error| {
            AppError::Internal(anyhow::anyhow!("Failed to serialize preferences: {error}"))
        })?;
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO user_preferences (user_id, preferences_json, updated_at) VALUES (?, ?, ?) ON CONFLICT(user_id) DO UPDATE SET preferences_json = excluded.preferences_json, updated_at = excluded.updated_at",
        )
        .bind(user_id)
        .bind(json)
        .bind(now)
        .execute(&self.db)
        .await
        .map_err(|error| AppError::Internal(anyhow::anyhow!("Failed to persist user preferences: {error}")))?;
        Ok(())
    }
}
