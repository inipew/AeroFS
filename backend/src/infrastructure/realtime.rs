use crate::db::DbPool;
use crate::errors::AppError;
use crate::ports::realtime::RealtimeAuthorization;
use async_trait::async_trait;
use std::collections::HashSet;

#[derive(Clone)]
pub struct SqliteRealtimeAuthorization {
    db: DbPool,
}

impl SqliteRealtimeAuthorization {
    pub fn new(db: DbPool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl RealtimeAuthorization for SqliteRealtimeAuthorization {
    async fn readable_connections(&self, user_id: &str) -> Result<HashSet<String>, AppError> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT connection_id FROM permissions WHERE user_id = ? AND can_read = 1",
        )
        .bind(user_id)
        .fetch_all(&self.db)
        .await
        .map_err(|error| AppError::Internal(anyhow::anyhow!(
            "failed to load realtime authorization snapshot: {error}"
        )))?;

        Ok(rows.into_iter().map(|(connection_id,)| connection_id).collect())
    }
}
