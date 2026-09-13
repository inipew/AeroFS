use crate::auth::session::UserInfo;
use crate::errors::AppError;
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct LoginIdentity {
    pub id: String,
    pub username: String,
    pub password_hash: String,
    pub is_admin: bool,
}

#[derive(Debug, Clone)]
pub struct UserAccountRecord {
    pub id: String,
    pub username: String,
    pub is_admin: bool,
    pub created_at: String,
    pub updated_at: String,
    pub permissions_count: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserMutationOutcome {
    Applied,
    NotFound,
    LastAdmin,
}

#[async_trait]
pub trait AccountRepository: Send + Sync {
    async fn login_identity(&self, username: &str) -> Result<Option<LoginIdentity>, AppError>;
    async fn list_users(&self) -> Result<Vec<UserAccountRecord>, AppError>;
    async fn get_user(&self, username: &str) -> Result<Option<UserAccountRecord>, AppError>;
    async fn create_user(
        &self,
        id: &str,
        username: &str,
        password_hash: &str,
        is_admin: bool,
        timestamp: &str,
    ) -> Result<(), AppError>;
    async fn update_password(
        &self,
        username: &str,
        password_hash: &str,
        timestamp: &str,
    ) -> Result<bool, AppError>;
    async fn delete_user_preserving_last_admin(
        &self,
        username: &str,
    ) -> Result<UserMutationOutcome, AppError>;
    async fn set_admin_role_preserving_last_admin(
        &self,
        username: &str,
        is_admin: bool,
        timestamp: &str,
    ) -> Result<UserMutationOutcome, AppError>;
}

#[async_trait]
pub trait SessionRepository: Send + Sync {
    async fn validate(&self, session_id: &str) -> Result<Option<UserInfo>, AppError>;
    async fn create(&self, user_id: &str, ttl_secs: u64) -> Result<String, AppError>;
    async fn delete(&self, session_id: &str) -> Result<(), AppError>;
}

#[async_trait]
pub trait AuthAudit: Send + Sync {
    #[allow(clippy::too_many_arguments)]
    async fn record(
        &self,
        user_id: Option<&str>,
        action: &str,
        status: &str,
        ip_address: Option<&str>,
        details: Option<&str>,
    );
}
