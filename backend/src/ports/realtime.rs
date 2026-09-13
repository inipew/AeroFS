use crate::errors::AppError;
use async_trait::async_trait;
use std::collections::HashSet;

#[async_trait]
pub trait RealtimeAuthorization: Send + Sync {
    async fn readable_connections(&self, user_id: &str) -> Result<HashSet<String>, AppError>;
}
