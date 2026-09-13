use crate::auth::audit::AuditLogEntry;
use crate::domain::settings::UserPreferences;
use crate::errors::AppError;
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct AuditRecord<'a> {
    pub user_id: Option<&'a str>,
    pub action: &'a str,
    pub connection_id: Option<&'a str>,
    pub resource_path: Option<&'a str>,
    pub status: &'a str,
    pub ip_address: Option<&'a str>,
    pub details: Option<&'a str>,
}

#[async_trait]
pub trait AuditRepository: Send + Sync {
    async fn record(&self, record: AuditRecord<'_>) -> Result<(), AppError>;
    async fn list(&self, limit: usize, offset: usize) -> Result<Vec<AuditLogEntry>, AppError>;
}

#[async_trait]
pub trait UserPreferencesRepository: Send + Sync {
    async fn get(&self, user_id: &str) -> Result<Option<UserPreferences>, AppError>;
    async fn set(&self, user_id: &str, prefs: &UserPreferences) -> Result<(), AppError>;
}
