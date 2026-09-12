use crate::auth::password::{hash_password, verify_password};
use crate::auth::AuthenticatedUser;
use crate::db::DbPool;
use crate::domain::{Actor, ConnectionId, VfsPath};
use crate::errors::AppError;
use crate::ports::{
    authorization::{Authorization, FileAction},
    filesystem::FileSystemResolver,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::io::AsyncReadExt;
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

pub struct PublicShareContent {
    pub name: String,
    pub data: Vec<u8>,
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

#[derive(Clone)]
pub struct ShareService {
    db: DbPool,
    authorization: Arc<dyn Authorization>,
    filesystem: Arc<dyn FileSystemResolver>,
}

impl ShareService {
    pub fn new(
        db: DbPool,
        authorization: Arc<dyn Authorization>,
        filesystem: Arc<dyn FileSystemResolver>,
    ) -> Self {
        Self {
            db,
            authorization,
            filesystem,
        }
    }

    pub async fn list_shares(
        &self,
        user: &AuthenticatedUser,
    ) -> Result<Vec<ShareItem>, AppError> {
        let rows: Vec<ShareDbRow> = if user.is_admin {
            sqlx::query_as(
                "SELECT id, connection_id, path, share_token, password_hash, expires_at, created_at
                 FROM shares
                 ORDER BY created_at DESC",
            )
            .fetch_all(&self.db)
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
            .fetch_all(&self.db)
            .await
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))?
        };

        Ok(rows
            .into_iter()
            .map(
                |(id, connection_id, path, share_token, pass_hash, expires_at, created_at)| {
                    ShareItem {
                        id,
                        connection_id,
                        path,
                        share_url: format!("/api/v1/shares/public/{}", share_token),
                        share_token,
                        has_password: pass_hash.is_some(),
                        expires_at,
                        created_at,
                    }
                },
            )
            .collect())
    }

    pub async fn create_share(
        &self,
        user: &AuthenticatedUser,
        payload: CreateShareRequest,
    ) -> Result<ShareItem, AppError> {
        let connection = ConnectionId::new(payload.connection_id.clone())
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        let actor = Actor {
            id: user.id.clone(),
            username: user.username.clone(),
            is_admin: user.is_admin,
        };
        self.authorization
            .authorize(&actor, &connection, FileAction::Read)
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
        let password_hash = match payload.password.as_ref().filter(|pwd| !pwd.trim().is_empty()) {
            Some(pwd) => Some(hash_password(pwd)?),
            None => None,
        };

        sqlx::query(
            "INSERT INTO shares (id, connection_id, path, share_token, password_hash, expires_at, created_at, created_by)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&payload.connection_id)
        .bind(&payload.path)
        .bind(&share_token)
        .bind(&password_hash)
        .bind(&expires_at_str)
        .bind(&now_str)
        .bind(&user.username)
        .execute(&self.db)
        .await
        .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

        Ok(ShareItem {
            id,
            connection_id: payload.connection_id,
            path: payload.path,
            share_url: format!("/api/v1/shares/public/{}", share_token),
            share_token,
            has_password: password_hash.is_some(),
            expires_at: expires_at_str,
            created_at: now_str,
        })
    }

    pub async fn delete_share(
        &self,
        user: &AuthenticatedUser,
        share_id: &str,
    ) -> Result<(), AppError> {
        let res = if user.is_admin {
            sqlx::query("DELETE FROM shares WHERE id = ?")
                .bind(share_id)
                .execute(&self.db)
                .await
        } else {
            sqlx::query("DELETE FROM shares WHERE id = ? AND created_by = ?")
                .bind(share_id)
                .bind(&user.username)
                .execute(&self.db)
                .await
        }
        .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

        if res.rows_affected() == 0 {
            return Err(AppError::NotFound("Share not found".into()));
        }
        Ok(())
    }

    async fn verify_share(
        &self,
        token: &str,
        password: Option<&str>,
    ) -> Result<(String, String), AppError> {
        let row: Option<(String, String, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT connection_id, path, password_hash, expires_at FROM shares WHERE share_token = ?",
        )
        .bind(token)
        .fetch_optional(&self.db)
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
            if !verify_password(password.unwrap_or(""), &hash) {
                return Err(AppError::Unauthorized(
                    "Password required or incorrect".into(),
                ));
            }
        }

        let _ = sqlx::query(
            "UPDATE shares SET download_count = download_count + 1, last_accessed_at = ? WHERE share_token = ?",
        )
        .bind(Utc::now().to_rfc3339())
        .bind(token)
        .execute(&self.db)
        .await;

        Ok((connection_id, path))
    }

    pub async fn verify_and_get_public_share(
        &self,
        token: &str,
        password: Option<&str>,
    ) -> Result<(String, String), AppError> {
        self.verify_share(token, password).await
    }

    pub async fn read_public_share(
        &self,
        token: &str,
        password: Option<&str>,
    ) -> Result<PublicShareContent, AppError> {
        let (connection_id, path) = self.verify_share(token, password).await?;
        let connection = ConnectionId::new(connection_id.clone())
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        let provider = self
            .filesystem
            .resolve(&connection)
            .await
            .map_err(|_| AppError::NotFound("Storage connection not available".into()))?;
        let vfs_path = VfsPath::new(&connection_id, &path)?;
        let metadata = provider.stat(&vfs_path).await?;
        let mut stream = provider.read_stream(&vfs_path).await?;
        let mut data = Vec::new();
        stream
            .read_to_end(&mut data)
            .await
            .map_err(|e| anyhow::anyhow!("Read error: {}", e))?;
        Ok(PublicShareContent {
            name: metadata.name,
            data,
        })
    }
}
