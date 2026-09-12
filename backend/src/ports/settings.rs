use crate::errors::AppError;
use async_trait::async_trait;

#[async_trait]
pub trait FileSettings: Send + Sync {
    async fn show_hidden_default(&self) -> Result<bool, AppError>;
    async fn max_editable_size(&self) -> Result<u64, AppError>;
    fn max_directory_entries(&self) -> usize;
}
