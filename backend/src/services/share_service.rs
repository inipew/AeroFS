use crate::auth::password::{hash_password, verify_password};
use crate::auth::permissions::{check_permission, PermissionAction};
use crate::auth::AuthenticatedUser;
use crate::errors::AppError;
use crate::state::AppState;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct ShareItem {
    pub id: String,
    pub connection_id: String,
    pub path: String,
    pub share_token: String,
    pub has_password: bool,
    pub expires_at: Option<String>,
    pub created_at: String,
    pub share_url: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateShareRequest {
    pub connection_id: String,
    pub path: String,
    pub password: Option<String>,
    pub expires_in_hours: Option<i64>,
}

type ShareDbRow = (
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    String,
);

pub struct ShareService;

impl ShareService {
    pub async fn list_shares(
        state: &AppState,
        user: &AuthenticatedUser,
    ) -> Result<Vec<ShareItem>, AppError> {
        let rows: Vec<ShareDbRow> = if user.is_admin {
            sqlx::query_as(
                "SELECT id, connection_id, path, share_token, password_hash, expires_at, created_at
                 FROM shares
                 ORDER BY created_at DESC",
            )
            .fetch_all(&state.db)
            .await
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))?
        } else {
            sqlx::query_as(
                "SELECT id, connection_id, path, share_token, password_hash, expires_at, created_at
                 FROM shares
                 WHERE created_by = ?
                 ORDER BY created_at DESC",
            )
            .bind(&user.username)
            .fetch_all(&state.db)
            .await
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))?
        };

        let shares = rows
            .into_iter()
            .map(
                |(id, connection_id, path, share_token, pass_hash, expires_at, created_at)| {
                    let share_url = format!("/api/v1/shares/public/{}", share_token);
                    ShareItem {
                        id,
                        connection_id,
                        path,
                        share_token,
                        has_password: pass_hash.is_some(),
                        expires_at,
                        created_at,
                        share_url,
                    }
                },
            )
            .collect();

        Ok(shares)
    }

    pub async fn create_share(
        state: &AppState,
        user: &AuthenticatedUser,
        payload: CreateShareRequest,
    ) -> Result<ShareItem, AppError> {
        check_permission(
            &state.db,
            user,
            &payload.connection_id,
            PermissionAction::Read,
        )
        .await?;

        let id = Uuid::new_v4().to_string();
        let share_token = format!(
            "{}{}",
            &Uuid::new_v4().to_string().replace('-', "")[..16],
            &Uuid::new_v4().to_string().replace('-', "")[..16]
        );
        let now = Utc::now();
        let now_str = now.to_rfc3339();

        let expires_at_str = payload
            .expires_in_hours
            .map(|h| (now + Duration::hours(h)).to_rfc3339());

        let password_hash = if let Some(ref pwd) = payload.password {
            if !pwd.trim().is_empty() {
                Some(hash_password(pwd)?)
            } else {
                None
            }
        } else {
            None
        };

        sqlx::query(
            "INSERT INTO shares (id, connection_id, path, share_token, password_hash, expires_at, created_at, created_by)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(&payload.connection_id)
        .bind(&payload.path)
        .bind(&share_token)
        .bind(&password_hash)
        .bind(&expires_at_str)
        .bind(&now_str)
        .bind(&user.username)
        .execute(&state.db)
        .await
        .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

        let share_url = format!("/api/v1/shares/public/{}", share_token);
        Ok(ShareItem {
            id,
            connection_id: payload.connection_id,
            path: payload.path,
            share_token,
            has_password: password_hash.is_some(),
            expires_at: expires_at_str,
            created_at: now_str,
            share_url,
        })
    }

    pub async fn delete_share(
        state: &AppState,
        user: &AuthenticatedUser,
        share_id: &str,
    ) -> Result<(), AppError> {
        let res = if user.is_admin {
            sqlx::query("DELETE FROM shares WHERE id = ?")
                .bind(share_id)
                .execute(&state.db)
                .await
        } else {
            sqlx::query("DELETE FROM shares WHERE id = ? AND created_by = ?")
                .bind(share_id)
                .bind(&user.username)
                .execute(&state.db)
                .await
        }
        .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

        if res.rows_affected() == 0 {
            return Err(AppError::NotFound("Share not found".into()));
        }

        Ok(())
    }

    pub async fn verify_and_get_public_share(
        state: &AppState,
        token: &str,
        password: Option<&str>,
    ) -> Result<(String, String), AppError> {
        let row: Option<(String, String, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT connection_id, path, password_hash, expires_at FROM shares WHERE share_token = ?"
        )
        .bind(token)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

        let (connection_id, path, password_hash, expires_at) =
            row.ok_or_else(|| AppError::NotFound("Shared link not found or expired".into()))?;

        if let Some(exp) = expires_at {
            if let Ok(exp_dt) = DateTime::parse_from_rfc3339(&exp) {
                if exp_dt.with_timezone(&Utc) < Utc::now() {
                    return Err(AppError::NotFound("Shared link has expired".into()));
                }
            }
        }

        if let Some(hash) = password_hash {
            let pwd = password.unwrap_or("");
            if !verify_password(pwd, &hash) {
                return Err(AppError::Unauthorized(
                    "Password required or incorrect".into(),
                ));
            }
        }

        let _ = sqlx::query(
            "UPDATE shares SET download_count = download_count + 1, last_accessed_at = ? WHERE share_token = ?"
        )
        .bind(Utc::now().to_rfc3339())
        .bind(token)
        .execute(&state.db)
        .await;

        Ok((connection_id, path))
    }
}
