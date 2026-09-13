use crate::db::DbPool;
use crate::errors::AppError;
use crate::ports::trash::{NewTrashRecord, TrashRecord, TrashRepository};
use async_trait::async_trait;

#[derive(Clone)]
pub struct SqliteTrashRepository {
    db: DbPool,
}

impl SqliteTrashRepository {
    pub fn new(db: DbPool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl TrashRepository for SqliteTrashRepository {
    async fn list(&self) -> Result<Vec<TrashRecord>, AppError> {
        let rows: Vec<(String, String, String, String, String, i64, Option<i64>, String)> =
            sqlx::query_as(
                "SELECT id, connection_id, original_path, trash_path, item_name, is_directory, size, deleted_at FROM trash_items ORDER BY deleted_at DESC",
            )
            .fetch_all(&self.db)
            .await
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

        Ok(rows
            .into_iter()
            .map(
                |(id, connection_id, original_path, trash_path, item_name, is_dir, size, deleted_at)| TrashRecord {
                    id,
                    connection_id,
                    original_path,
                    trash_path,
                    item_name,
                    is_directory: is_dir != 0,
                    size,
                    deleted_at,
                },
            )
            .collect())
    }

    async fn insert(&self, record: &NewTrashRecord) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO trash_items (id, connection_id, original_path, trash_path, item_name, is_directory, size, deleted_at, deleted_by)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&record.id)
        .bind(&record.connection_id)
        .bind(&record.original_path)
        .bind(&record.trash_path)
        .bind(&record.item_name)
        .bind(i64::from(record.is_directory))
        .bind(record.size)
        .bind(&record.deleted_at)
        .bind(&record.deleted_by)
        .execute(&self.db)
        .await
        .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;
        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Option<TrashRecord>, AppError> {
        let row: Option<(String, String, String, String, String, i64, Option<i64>, String)> =
            sqlx::query_as(
                "SELECT id, connection_id, original_path, trash_path, item_name, is_directory, size, deleted_at FROM trash_items WHERE id = ?",
            )
            .bind(id)
            .fetch_optional(&self.db)
            .await
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

        Ok(row.map(
            |(id, connection_id, original_path, trash_path, item_name, is_dir, size, deleted_at)| TrashRecord {
                id,
                connection_id,
                original_path,
                trash_path,
                item_name,
                is_directory: is_dir != 0,
                size,
                deleted_at,
            },
        ))
    }

    async fn delete(&self, id: &str) -> Result<bool, AppError> {
        let result = sqlx::query("DELETE FROM trash_items WHERE id = ?")
            .bind(id)
            .execute(&self.db)
            .await
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;
        Ok(result.rows_affected() > 0)
    }
}
