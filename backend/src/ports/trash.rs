use crate::errors::AppError;
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct TrashRecord {
    pub id: String,
    pub connection_id: String,
    pub original_path: String,
    pub trash_path: String,
    pub item_name: String,
    pub is_directory: bool,
    pub size: Option<i64>,
    pub deleted_at: String,
}

#[derive(Debug, Clone)]
pub struct NewTrashRecord {
    pub id: String,
    pub connection_id: String,
    pub original_path: String,
    pub trash_path: String,
    pub item_name: String,
    pub is_directory: bool,
    pub size: i64,
    pub deleted_at: String,
    pub deleted_by: String,
}

#[async_trait]
pub trait TrashRepository: Send + Sync {
    async fn list(&self) -> Result<Vec<TrashRecord>, AppError>;
    async fn insert(&self, record: &NewTrashRecord) -> Result<(), AppError>;
    async fn get(&self, id: &str) -> Result<Option<TrashRecord>, AppError>;
    async fn delete(&self, id: &str) -> Result<bool, AppError>;
}
