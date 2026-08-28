use crate::auth::password::hash_password;
use crate::db::DbPool;
use crate::errors::AppError;
use chrono::Utc;
use serde::Serialize;
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct UserSummary {
    pub id: String,
    pub username: String,
    pub is_admin: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UserDetail {
    pub id: String,
    pub username: String,
    pub is_admin: bool,
    pub created_at: String,
    pub updated_at: String,
    pub permissions_count: i64,
}

pub struct UserService;

impl UserService {
    /// List all registered system users
    pub async fn list_users(pool: &DbPool) -> Result<Vec<UserSummary>, AppError> {
        let rows = sqlx::query(
            "SELECT id, username, is_admin, created_at, updated_at FROM users ORDER BY created_at ASC",
        )
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to list users: {}", e)))?;

        let users = rows
            .into_iter()
            .map(|r| UserSummary {
                id: r.get("id"),
                username: r.get("username"),
                is_admin: r.get::<i64, _>("is_admin") == 1,
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
            })
            .collect();

        Ok(users)
    }

    /// Retrieve details for a specific user
    pub async fn get_user(pool: &DbPool, username: &str) -> Result<UserDetail, AppError> {
        let row = sqlx::query(
            "SELECT id, username, is_admin, created_at, updated_at FROM users WHERE username = ?",
        )
        .bind(username)
        .fetch_optional(pool)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Database error: {}", e)))?;

        let r = row.ok_or_else(|| AppError::NotFound(format!("User '{}' not found", username)))?;

        let user_id: String = r.get("id");
        let perm_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM permissions WHERE user_id = ?")
                .bind(&user_id)
                .fetch_one(pool)
                .await
                .unwrap_or((0,));

        Ok(UserDetail {
            id: user_id,
            username: r.get("username"),
            is_admin: r.get::<i64, _>("is_admin") == 1,
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
            permissions_count: perm_count.0,
        })
    }

    /// Create a new user with secure password hash
    pub async fn create_user(
        pool: &DbPool,
        username: &str,
        password: &str,
        is_admin: bool,
    ) -> Result<String, AppError> {
        let username_clean = username.trim();
        if username_clean.is_empty() {
            return Err(AppError::BadRequest("Username cannot be empty".into()));
        }

        if password.len() < 4 {
            return Err(AppError::BadRequest(
                "Password must be at least 4 characters long".into(),
            ));
        }

        // Check if user already exists
        let exists: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM users WHERE username = ?")
            .bind(username_clean)
            .fetch_optional(pool)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("DB error: {}", e)))?;

        if exists.is_some() {
            return Err(AppError::Conflict(format!(
                "User '{}' already exists",
                username_clean
            )));
        }

        let hashed = hash_password(password)?;
        let uid = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO users (id, username, password_hash, is_admin, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&uid)
        .bind(username_clean)
        .bind(&hashed)
        .bind(if is_admin { 1 } else { 0 })
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to insert user: {}", e)))?;

        Ok(uid)
    }

    /// Update user's password
    pub async fn update_password(
        pool: &DbPool,
        username: &str,
        new_password: &str,
    ) -> Result<(), AppError> {
        if new_password.len() < 4 {
            return Err(AppError::BadRequest(
                "Password must be at least 4 characters long".into(),
            ));
        }

        let hashed = hash_password(new_password)?;
        let now = Utc::now().to_rfc3339();

        let res =
            sqlx::query("UPDATE users SET password_hash = ?, updated_at = ? WHERE username = ?")
                .bind(&hashed)
                .bind(&now)
                .bind(username)
                .execute(pool)
                .await
                .map_err(|e| {
                    AppError::Internal(anyhow::anyhow!("Failed to update password: {}", e))
                })?;

        if res.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("User '{}' not found", username)));
        }

        Ok(())
    }

    /// Delete user with safeguard for the last remaining administrator
    pub async fn delete_user(pool: &DbPool, username: &str) -> Result<(), AppError> {
        let user = Self::get_user(pool, username).await?;

        if user.is_admin {
            let admin_count: (i64,) =
                sqlx::query_as("SELECT COUNT(*) FROM users WHERE is_admin = 1")
                    .fetch_one(pool)
                    .await
                    .map_err(|e| AppError::Internal(anyhow::anyhow!("DB error: {}", e)))?;

            if admin_count.0 <= 1 {
                return Err(AppError::Forbidden(
                    "Cannot delete the last administrator account. At least one administrator must remain.".into(),
                ));
            }
        }

        // Delete user's sessions and permissions before removing user
        let _ = sqlx::query("DELETE FROM sessions WHERE user_id = ?")
            .bind(&user.id)
            .execute(pool)
            .await;
        let _ = sqlx::query("DELETE FROM permissions WHERE user_id = ?")
            .bind(&user.id)
            .execute(pool)
            .await;

        let res = sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(&user.id)
            .execute(pool)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to delete user: {}", e)))?;

        if res.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("User '{}' not found", username)));
        }

        Ok(())
    }

    /// Promote or demote user administrator role with last-admin safeguard
    pub async fn set_admin_role(
        pool: &DbPool,
        username: &str,
        is_admin: bool,
    ) -> Result<(), AppError> {
        let user = Self::get_user(pool, username).await?;

        if !is_admin && user.is_admin {
            // Demoting an admin: verify there is at least one other admin
            let admin_count: (i64,) =
                sqlx::query_as("SELECT COUNT(*) FROM users WHERE is_admin = 1")
                    .fetch_one(pool)
                    .await
                    .map_err(|e| AppError::Internal(anyhow::anyhow!("DB error: {}", e)))?;

            if admin_count.0 <= 1 {
                return Err(AppError::Forbidden(
                    "Cannot demote the last administrator account. At least one administrator must remain.".into(),
                ));
            }
        }

        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE users SET is_admin = ?, updated_at = ? WHERE id = ?")
            .bind(if is_admin { 1 } else { 0 })
            .bind(&now)
            .bind(&user.id)
            .execute(pool)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to update role: {}", e)))?;

        Ok(())
    }
}
