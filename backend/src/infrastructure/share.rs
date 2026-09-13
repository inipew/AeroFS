use crate::db::DbPool;
use crate::errors::AppError;
use crate::ports::share::{NewShareRecord, ShareRecord, ShareRepository};
use async_trait::async_trait;

#[derive(Clone)]
pub struct SqliteShareRepository {
    db: DbPool,
}

impl SqliteShareRepository {
    pub fn new(db: DbPool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ShareRepository for SqliteShareRepository {
    async fn list(&self, owner: Option<&str>) -> Result<Vec<ShareRecord>, AppError> {
        let rows: Vec<(String, String, String, String, Option<String>, Option<String>, String)> =
            if let Some(owner) = owner {
                sqlx::query_as(
                    "SELECT id, connection_id, path, share_token, password_hash, expires_at, created_at
                     FROM shares WHERE created_by = ? ORDER BY created_at DESC",
                )
                .bind(owner)
                .fetch_all(&self.db)
                .await
            } else {
                sqlx::query_as(
                    "SELECT id, connection_id, path, share_token, password_hash, expires_at, created_at
                     FROM shares ORDER BY created_at DESC",
                )
                .fetch_all(&self.db)
                .await
            }
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

        Ok(rows
            .into_iter()
            .map(|(id, connection_id, path, share_token, password_hash, expires_at, created_at)| {
                ShareRecord {
                    id,
                    connection_id,
                    path,
                    share_token,
                    password_hash,
                    expires_at,
                    created_at,
                }
            })
            .collect())
    }

    async fn insert(&self, record: &NewShareRecord) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO shares (id, connection_id, path, share_token, password_hash, expires_at, created_at, created_by)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&record.id)
        .bind(&record.connection_id)
        .bind(&record.path)
        .bind(&record.share_token)
        .bind(&record.password_hash)
        .bind(&record.expires_at)
        .bind(&record.created_at)
        .bind(&record.created_by)
        .execute(&self.db)
        .await
        .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;
        Ok(())
    }

    async fn delete(&self, id: &str, owner: Option<&str>) -> Result<bool, AppError> {
        let result = if let Some(owner) = owner {
            sqlx::query("DELETE FROM shares WHERE id = ? AND created_by = ?")
                .bind(id)
                .bind(owner)
                .execute(&self.db)
                .await
        } else {
            sqlx::query("DELETE FROM shares WHERE id = ?")
                .bind(id)
                .execute(&self.db)
                .await
        }
        .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;
        Ok(result.rows_affected() > 0)
    }

    async fn get_by_token(&self, token: &str) -> Result<Option<ShareRecord>, AppError> {
        let row: Option<(String, String, String, String, Option<String>, Option<String>, String)> =
            sqlx::query_as(
                "SELECT id, connection_id, path, share_token, password_hash, expires_at, created_at
                 FROM shares WHERE share_token = ?",
            )
            .bind(token)
            .fetch_optional(&self.db)
            .await
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;
        Ok(row.map(|(id, connection_id, path, share_token, password_hash, expires_at, created_at)| {
            ShareRecord {
                id,
                connection_id,
                path,
                share_token,
                password_hash,
                expires_at,
                created_at,
            }
        }))
    }

    async fn record_access(&self, token: &str, accessed_at: &str) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE shares SET download_count = download_count + 1, last_accessed_at = ? WHERE share_token = ?",
        )
        .bind(accessed_at)
        .bind(token)
        .execute(&self.db)
        .await
        .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;
        Ok(())
    }
}
