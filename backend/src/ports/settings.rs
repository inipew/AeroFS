use crate::domain::Actor;
use crate::errors::AppError;
use async_trait::async_trait;
use std::path::PathBuf;

#[async_trait]
pub trait FileSettings: Send + Sync {
    async fn show_hidden_default(&self) -> Result<bool, AppError>;
    async fn max_editable_size(&self) -> Result<u64, AppError>;
    fn max_directory_entries(&self) -> usize;
}

#[async_trait]
pub trait SystemSettingsStore: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<String>, AppError>;
    async fn upsert_many(&self, values: &[(String, String)]) -> Result<(), AppError>;
}

#[async_trait]
pub trait PreparedLocalRoot: Send {
    async fn activate(self: Box<Self>);
}

#[async_trait]
pub trait SettingsRuntime: Send + Sync {
    async fn prepare_local_root(
        &self,
        new_root: PathBuf,
    ) -> Result<Box<dyn PreparedLocalRoot>, AppError>;

    fn set_max_concurrent_transfers(&self, limit: usize);
}

#[async_trait]
pub trait SettingsAudit: Send + Sync {
    async fn settings_updated(&self, actor: &Actor);
}
