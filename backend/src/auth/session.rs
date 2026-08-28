use crate::db::DbPool;
use crate::errors::{AppError, AuthError};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub is_admin: bool,
}

pub async fn create_session(
    pool: &DbPool,
    user_id: &str,
    ttl_secs: u64,
) -> Result<String, AppError> {
    let session_id = Uuid::new_v4().to_string();
    let now = Utc::now();
    let expires_at = now + Duration::seconds(ttl_secs as i64);

    sqlx::query("INSERT INTO sessions (id, user_id, expires_at, created_at) VALUES (?, ?, ?, ?)")
        .bind(&session_id)
        .bind(user_id)
        .bind(expires_at.to_rfc3339())
        .bind(now.to_rfc3339())
        .execute(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create session: {}", e))?;

    Ok(session_id)
}

pub async fn validate_session(
    pool: &DbPool,
    session_id: &str,
) -> Result<Option<UserInfo>, AppError> {
    let row: Option<(String, String, i64, String)> = sqlx::query_as(
        "SELECT u.id, u.username, u.is_admin, s.expires_at 
         FROM sessions s
         JOIN users u ON s.user_id = u.id
         WHERE s.id = ?",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to query session: {}", e))?;

    if let Some((id, username, is_admin, expires_at_str)) = row {
        if let Ok(exp) = DateTime::parse_from_rfc3339(&expires_at_str) {
            if exp.with_timezone(&Utc) < Utc::now() {
                // Expired session: delete it
                let _ = delete_session(pool, session_id).await;
                return Err(AppError::Auth(AuthError::SessionExpired));
            }
        }

        Ok(Some(UserInfo {
            id,
            username,
            is_admin: is_admin != 0,
        }))
    } else {
        Ok(None)
    }
}

pub async fn delete_session(pool: &DbPool, session_id: &str) -> Result<(), AppError> {
    sqlx::query("DELETE FROM sessions WHERE id = ?")
        .bind(session_id)
        .execute(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to delete session: {}", e))?;
    Ok(())
}
