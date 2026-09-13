use crate::errors::AppError;
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct ShareRecord {
    pub id: String,
    pub connection_id: String,
    pub path: String,
    pub share_token: String,
    pub password_hash: Option<String>,
    pub expires_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct NewShareRecord {
    pub id: String,
    pub connection_id: String,
    pub path: String,
    pub share_token: String,
    pub password_hash: Option<String>,
    pub expires_at: Option<String>,
    pub created_at: String,
    pub created_by: String,
}

#[async_trait]
pub trait ShareRepository: Send + Sync {
    async fn list(&self, owner: Option<&str>) -> Result<Vec<ShareRecord>, AppError>;
    async fn insert(&self, record: &NewShareRecord) -> Result<(), AppError>;
    async fn delete(&self, id: &str, owner: Option<&str>) -> Result<bool, AppError>;
    async fn get_by_token(&self, token: &str) -> Result<Option<ShareRecord>, AppError>;
    async fn record_access(&self, token: &str, accessed_at: &str) -> Result<(), AppError>;
}
