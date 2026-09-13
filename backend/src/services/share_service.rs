use crate::auth::password::{hash_password, verify_password};
use crate::auth::AuthenticatedUser;
use crate::domain::{Actor, ConnectionId, VfsPath};
use crate::errors::AppError;
use crate::ports::{
    authorization::{Authorization, FileAction},
    filesystem::FileSystemResolver,
    share::{NewShareRecord, ShareRepository},
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

#[derive(Clone)]
pub struct ShareService {
    repository: Arc<dyn ShareRepository>,
    authorization: Arc<dyn Authorization>,
    filesystem: Arc<dyn FileSystemResolver>,
}

impl ShareService {
    pub fn new(
        repository: Arc<dyn ShareRepository>,
        authorization: Arc<dyn Authorization>,
        filesystem: Arc<dyn FileSystemResolver>,
    ) -> Self {
        Self {
            repository,
            authorization,
            filesystem,
        }
    }

    pub async fn list_shares(
        &self,
        user: &AuthenticatedUser,
    ) -> Result<Vec<ShareItem>, AppError> {
        let owner = (!user.is_admin).then_some(user.username.as_str());
        Ok(self
            .repository
            .list(owner)
            .await?
            .into_iter()
            .map(|record| ShareItem {
                id: record.id,
                connection_id: record.connection_id,
                path: record.path,
                share_url: format!("/api/v1/shares/public/{}", record.share_token),
                share_token: record.share_token,
                has_password: record.password_hash.is_some(),
                expires_at: record.expires_at,
                created_at: record.created_at,
            })
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
        let expires_at = payload
            .expires_in_hours
            .map(|hours| (now + Duration::hours(hours)).to_rfc3339());
        let password_hash = match payload.password.as_ref().filter(|pwd| !pwd.trim().is_empty()) {
            Some(password) => Some(hash_password(password)?),
            None => None,
        };

        self.repository
            .insert(&NewShareRecord {
                id: id.clone(),
                connection_id: payload.connection_id.clone(),
                path: payload.path.clone(),
                share_token: share_token.clone(),
                password_hash: password_hash.clone(),
                expires_at: expires_at.clone(),
                created_at: now_str.clone(),
                created_by: user.username.clone(),
            })
            .await?;

        Ok(ShareItem {
            id,
            connection_id: payload.connection_id,
            path: payload.path,
            share_url: format!("/api/v1/shares/public/{}", share_token),
            share_token,
            has_password: password_hash.is_some(),
            expires_at,
            created_at: now_str,
        })
    }

    pub async fn delete_share(
        &self,
        user: &AuthenticatedUser,
        share_id: &str,
    ) -> Result<(), AppError> {
        let owner = (!user.is_admin).then_some(user.username.as_str());
        if !self.repository.delete(share_id, owner).await? {
            return Err(AppError::NotFound("Share not found".into()));
        }
        Ok(())
    }

    async fn verify_share(
        &self,
        token: &str,
        password: Option<&str>,
    ) -> Result<(String, String), AppError> {
        let record = self
            .repository
            .get_by_token(token)
            .await?
            .ok_or_else(|| AppError::NotFound("Shared link not found or expired".into()))?;

        if let Some(expires_at) = record.expires_at.as_deref() {
            let expires_at = DateTime::parse_from_rfc3339(expires_at).map_err(|error| {
                AppError::Internal(anyhow::anyhow!(
                    "share '{}' has invalid persisted expiration timestamp: {}",
                    record.id,
                    error
                ))
            })?;
            if expires_at.with_timezone(&Utc) < Utc::now() {
                return Err(AppError::NotFound("Shared link has expired".into()));
            }
        }

        if let Some(hash) = record.password_hash.as_deref() {
            if !verify_password(password.unwrap_or(""), hash) {
                return Err(AppError::Unauthorized(
                    "Password required or incorrect".into(),
                ));
            }
        }

        if let Err(error) = self
            .repository
            .record_access(token, &Utc::now().to_rfc3339())
            .await
        {
            tracing::warn!(%error, token, "failed to persist public share access accounting");
        }

        Ok((record.connection_id, record.path))
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
