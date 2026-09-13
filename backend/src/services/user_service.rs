use crate::auth::password::hash_password;
use crate::errors::AppError;
use crate::ports::auth::{AccountRepository, UserMutationOutcome};
use chrono::Utc;
use serde::Serialize;
use std::sync::Arc;
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

#[derive(Clone)]
pub struct UserService {
    repository: Arc<dyn AccountRepository>,
}

impl UserService {
    pub fn new(repository: Arc<dyn AccountRepository>) -> Self {
        Self { repository }
    }

    pub async fn list_users(&self) -> Result<Vec<UserSummary>, AppError> {
        Ok(self
            .repository
            .list_users()
            .await?
            .into_iter()
            .map(|user| UserSummary {
                id: user.id,
                username: user.username,
                is_admin: user.is_admin,
                created_at: user.created_at,
                updated_at: user.updated_at,
            })
            .collect())
    }

    pub async fn get_user(&self, username: &str) -> Result<UserDetail, AppError> {
        let user = self
            .repository
            .get_user(username)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("User '{}' not found", username)))?;
        Ok(UserDetail {
            id: user.id,
            username: user.username,
            is_admin: user.is_admin,
            created_at: user.created_at,
            updated_at: user.updated_at,
            permissions_count: user.permissions_count,
        })
    }

    pub async fn create_user(
        &self,
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

        let hashed = hash_password(password)?;
        let uid = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        self.repository
            .create_user(&uid, username_clean, &hashed, is_admin, &now)
            .await?;
        Ok(uid)
    }

    pub async fn update_password(
        &self,
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
        if !self
            .repository
            .update_password(username, &hashed, &now)
            .await?
        {
            return Err(AppError::NotFound(format!("User '{}' not found", username)));
        }
        Ok(())
    }

    pub async fn delete_user(&self, username: &str) -> Result<(), AppError> {
        match self
            .repository
            .delete_user_preserving_last_admin(username)
            .await?
        {
            UserMutationOutcome::Applied => Ok(()),
            UserMutationOutcome::NotFound => {
                Err(AppError::NotFound(format!("User '{}' not found", username)))
            }
            UserMutationOutcome::LastAdmin => Err(AppError::Forbidden(
                "Cannot delete the last administrator account. At least one administrator must remain."
                    .into(),
            )),
        }
    }

    pub async fn set_admin_role(&self, username: &str, is_admin: bool) -> Result<(), AppError> {
        let now = Utc::now().to_rfc3339();
        match self
            .repository
            .set_admin_role_preserving_last_admin(username, is_admin, &now)
            .await?
        {
            UserMutationOutcome::Applied => Ok(()),
            UserMutationOutcome::NotFound => {
                Err(AppError::NotFound(format!("User '{}' not found", username)))
            }
            UserMutationOutcome::LastAdmin => Err(AppError::Forbidden(
                "Cannot demote the last administrator account. At least one administrator must remain."
                    .into(),
            )),
        }
    }
}
