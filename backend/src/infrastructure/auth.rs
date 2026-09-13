use crate::auth::{audit::record_audit_log, session::UserInfo};
use crate::db::DbPool;
use crate::errors::{AppError, AuthError};
use crate::ports::auth::{
    AccountRepository, AuthAudit, LoginIdentity, SessionRepository, UserAccountRecord,
    UserMutationOutcome,
};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

#[derive(Clone)]
pub struct SqliteAccountRepository {
    db: DbPool,
}

impl SqliteAccountRepository {
    pub fn new(db: DbPool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl AccountRepository for SqliteAccountRepository {
    async fn login_identity(&self, username: &str) -> Result<Option<LoginIdentity>, AppError> {
        let row: Option<(String, String, String, i64)> = sqlx::query_as(
            "SELECT id, username, password_hash, is_admin FROM users WHERE username = ?",
        )
        .bind(username)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| anyhow::anyhow!("Database query error: {}", e))?;

        Ok(row.map(|(id, username, password_hash, is_admin)| LoginIdentity {
            id,
            username,
            password_hash,
            is_admin: is_admin != 0,
        }))
    }

    async fn list_users(&self) -> Result<Vec<UserAccountRecord>, AppError> {
        let rows: Vec<(String, String, i64, String, String, i64)> = sqlx::query_as(
            "SELECT u.id, u.username, u.is_admin, u.created_at, u.updated_at,
                    (SELECT COUNT(*) FROM permissions p WHERE p.user_id = u.id)
             FROM users u ORDER BY u.created_at ASC",
        )
        .fetch_all(&self.db)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to list users: {}", e))?;

        Ok(rows
            .into_iter()
            .map(|(id, username, is_admin, created_at, updated_at, permissions_count)| {
                UserAccountRecord {
                    id,
                    username,
                    is_admin: is_admin != 0,
                    created_at,
                    updated_at,
                    permissions_count,
                }
            })
            .collect())
    }

    async fn get_user(&self, username: &str) -> Result<Option<UserAccountRecord>, AppError> {
        let row: Option<(String, String, i64, String, String, i64)> = sqlx::query_as(
            "SELECT u.id, u.username, u.is_admin, u.created_at, u.updated_at,
                    (SELECT COUNT(*) FROM permissions p WHERE p.user_id = u.id)
             FROM users u WHERE u.username = ?",
        )
        .bind(username)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

        Ok(row.map(|(id, username, is_admin, created_at, updated_at, permissions_count)| {
            UserAccountRecord {
                id,
                username,
                is_admin: is_admin != 0,
                created_at,
                updated_at,
                permissions_count,
            }
        }))
    }

    async fn create_user(
        &self,
        id: &str,
        username: &str,
        password_hash: &str,
        is_admin: bool,
        timestamp: &str,
    ) -> Result<(), AppError> {
        let mut tx = self
            .db
            .begin()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to start user transaction: {}", e))?;
        let exists: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM users WHERE username = ?")
            .bind(username)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| anyhow::anyhow!("DB error: {}", e))?;
        if exists.is_some() {
            return Err(AppError::Conflict(format!("User '{}' already exists", username)));
        }

        sqlx::query(
            "INSERT INTO users (id, username, password_hash, is_admin, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(username)
        .bind(password_hash)
        .bind(i64::from(is_admin))
        .bind(timestamp)
        .bind(timestamp)
        .execute(&mut *tx)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to insert user: {}", e))?;
        tx.commit()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to commit user creation: {}", e))?;
        Ok(())
    }

    async fn update_password(
        &self,
        username: &str,
        password_hash: &str,
        timestamp: &str,
    ) -> Result<bool, AppError> {
        let result = sqlx::query(
            "UPDATE users SET password_hash = ?, updated_at = ? WHERE username = ?",
        )
        .bind(password_hash)
        .bind(timestamp)
        .bind(username)
        .execute(&self.db)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to update password: {}", e))?;
        Ok(result.rows_affected() > 0)
    }

    async fn delete_user_preserving_last_admin(
        &self,
        username: &str,
    ) -> Result<UserMutationOutcome, AppError> {
        let mut tx = self
            .db
            .begin()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to start user deletion transaction: {}", e))?;
        let user: Option<(String, i64)> =
            sqlx::query_as("SELECT id, is_admin FROM users WHERE username = ?")
                .bind(username)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| anyhow::anyhow!("DB error: {}", e))?;
        let Some((user_id, is_admin)) = user else {
            return Ok(UserMutationOutcome::NotFound);
        };

        if is_admin != 0 {
            let (admin_count,): (i64,) =
                sqlx::query_as("SELECT COUNT(*) FROM users WHERE is_admin = 1")
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(|e| anyhow::anyhow!("DB error: {}", e))?;
            if admin_count <= 1 {
                return Ok(UserMutationOutcome::LastAdmin);
            }
        }

        sqlx::query("DELETE FROM sessions WHERE user_id = ?")
            .bind(&user_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to delete user sessions: {}", e))?;
        sqlx::query("DELETE FROM permissions WHERE user_id = ?")
            .bind(&user_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to delete user permissions: {}", e))?;
        let result = sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(&user_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to delete user: {}", e))?;
        if result.rows_affected() == 0 {
            return Ok(UserMutationOutcome::NotFound);
        }
        tx.commit()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to commit user deletion: {}", e))?;
        Ok(UserMutationOutcome::Applied)
    }

    async fn set_admin_role_preserving_last_admin(
        &self,
        username: &str,
        is_admin: bool,
        timestamp: &str,
    ) -> Result<UserMutationOutcome, AppError> {
        let mut tx = self
            .db
            .begin()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to start role update transaction: {}", e))?;
        let user: Option<(String, i64)> =
            sqlx::query_as("SELECT id, is_admin FROM users WHERE username = ?")
                .bind(username)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| anyhow::anyhow!("DB error: {}", e))?;
        let Some((user_id, currently_admin)) = user else {
            return Ok(UserMutationOutcome::NotFound);
        };

        if !is_admin && currently_admin != 0 {
            let (admin_count,): (i64,) =
                sqlx::query_as("SELECT COUNT(*) FROM users WHERE is_admin = 1")
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(|e| anyhow::anyhow!("DB error: {}", e))?;
            if admin_count <= 1 {
                return Ok(UserMutationOutcome::LastAdmin);
            }
        }

        sqlx::query("UPDATE users SET is_admin = ?, updated_at = ? WHERE id = ?")
            .bind(i64::from(is_admin))
            .bind(timestamp)
            .bind(&user_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to update role: {}", e))?;
        tx.commit()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to commit role update: {}", e))?;
        Ok(UserMutationOutcome::Applied)
    }
}

#[derive(Clone)]
pub struct SqliteSessionRepository {
    db: DbPool,
}

impl SqliteSessionRepository {
    pub fn new(db: DbPool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl SessionRepository for SqliteSessionRepository {
    async fn validate(&self, session_id: &str) -> Result<Option<UserInfo>, AppError> {
        let row: Option<(String, String, i64, String)> = sqlx::query_as(
            "SELECT u.id, u.username, u.is_admin, s.expires_at
             FROM sessions s JOIN users u ON s.user_id = u.id WHERE s.id = ?",
        )
        .bind(session_id)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to query session: {}", e))?;

        let Some((id, username, is_admin, expires_at)) = row else {
            return Ok(None);
        };
        if DateTime::parse_from_rfc3339(&expires_at)
            .map(|value| value.with_timezone(&Utc) < Utc::now())
            .unwrap_or(false)
        {
            self.delete(session_id).await?;
            return Err(AppError::Auth(AuthError::SessionExpired));
        }
        Ok(Some(UserInfo {
            id,
            username,
            is_admin: is_admin != 0,
        }))
    }

    async fn create(&self, user_id: &str, ttl_secs: u64) -> Result<String, AppError> {
        let session_id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let expires_at = now + Duration::seconds(ttl_secs as i64);
        sqlx::query("INSERT INTO sessions (id, user_id, expires_at, created_at) VALUES (?, ?, ?, ?)")
            .bind(&session_id)
            .bind(user_id)
            .bind(expires_at.to_rfc3339())
            .bind(now.to_rfc3339())
            .execute(&self.db)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create session: {}", e))?;
        Ok(session_id)
    }

    async fn delete(&self, session_id: &str) -> Result<(), AppError> {
        sqlx::query("DELETE FROM sessions WHERE id = ?")
            .bind(session_id)
            .execute(&self.db)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to delete session: {}", e))?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct SqliteAuthAudit {
    db: DbPool,
}

impl SqliteAuthAudit {
    pub fn new(db: DbPool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl AuthAudit for SqliteAuthAudit {
    async fn record(
        &self,
        user_id: Option<&str>,
        action: &str,
        status: &str,
        ip_address: Option<&str>,
        details: Option<&str>,
    ) {
        record_audit_log(
            &self.db,
            user_id,
            action,
            None,
            None,
            status,
            ip_address,
            details,
        )
        .await;
    }
}
