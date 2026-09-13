use crate::auth::audit::record_audit_log;
use crate::db::DbPool;
use crate::domain::Actor;
use crate::errors::AppError;
use crate::ports::settings::{
    PreparedLocalRoot, SettingsAudit, SettingsRuntime, SystemSettingsStore,
};
use crate::transfer::TransferManager;
use crate::vfs::{factory::ProviderFactory, registry::ProviderRegistry, FileSystem};
use async_trait::async_trait;
use chrono::Utc;
use std::{path::PathBuf, sync::Arc};

#[derive(Clone)]
pub struct SqliteSystemSettingsStore {
    db: DbPool,
}

impl SqliteSystemSettingsStore {
    pub fn new(db: DbPool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl SystemSettingsStore for SqliteSystemSettingsStore {
    async fn get(&self, key: &str) -> Result<Option<String>, AppError> {
        sqlx::query_scalar("SELECT value FROM system_settings WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.db)
            .await
            .map_err(|error| AppError::Internal(anyhow::anyhow!("DB error: {error}")))
    }

    async fn upsert_many(&self, values: &[(String, String)]) -> Result<(), AppError> {
        if values.is_empty() {
            return Ok(());
        }

        let mut tx = self.db.begin().await.map_err(|error| {
            AppError::Internal(anyhow::anyhow!(
                "Failed to begin settings transaction: {error}"
            ))
        })?;
        let now = Utc::now().to_rfc3339();

        for (key, value) in values {
            sqlx::query(
                "INSERT INTO system_settings (key, value, updated_at) VALUES (?, ?, ?)\n                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            )
            .bind(key)
            .bind(value)
            .bind(&now)
            .execute(&mut *tx)
            .await
            .map_err(|error| AppError::Internal(anyhow::anyhow!("DB error: {error}")))?;
        }

        tx.commit().await.map_err(|error| {
            AppError::Internal(anyhow::anyhow!(
                "Failed to commit settings transaction: {error}"
            ))
        })
    }
}

pub struct PreparedRegistryLocalRoot {
    registry: Arc<ProviderRegistry>,
    provider: Arc<dyn FileSystem>,
}

#[async_trait]
impl PreparedLocalRoot for PreparedRegistryLocalRoot {
    async fn activate(self: Box<Self>) {
        self.registry
            .register("local".to_string(), self.provider.clone())
            .await;
    }
}

#[derive(Clone)]
pub struct RegistrySettingsRuntime {
    registry: Arc<ProviderRegistry>,
    transfer_manager: TransferManager,
}

impl RegistrySettingsRuntime {
    pub fn new(registry: Arc<ProviderRegistry>, transfer_manager: TransferManager) -> Self {
        Self {
            registry,
            transfer_manager,
        }
    }
}

#[async_trait]
impl SettingsRuntime for RegistrySettingsRuntime {
    async fn prepare_local_root(
        &self,
        new_root: PathBuf,
    ) -> Result<Box<dyn PreparedLocalRoot>, AppError> {
        tokio::fs::create_dir_all(&new_root).await.map_err(|error| {
            AppError::Internal(anyhow::anyhow!(
                "Failed to prepare local storage root '{}': {error}",
                new_root.display()
            ))
        })?;
        let provider = ProviderFactory::build_local("local", new_root).map_err(AppError::from)?;
        Ok(Box::new(PreparedRegistryLocalRoot {
            registry: self.registry.clone(),
            provider,
        }))
    }

    fn set_max_concurrent_transfers(&self, limit: usize) {
        self.transfer_manager.set_max_concurrent_transfers(limit);
    }
}

#[derive(Clone)]
pub struct SqliteSettingsAudit {
    db: DbPool,
}

impl SqliteSettingsAudit {
    pub fn new(db: DbPool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl SettingsAudit for SqliteSettingsAudit {
    async fn settings_updated(&self, actor: &Actor) {
        record_audit_log(
            &self.db,
            Some(&actor.id),
            "SETTINGS_UPDATED",
            None,
            None,
            "success",
            None,
            Some("Updated system settings"),
        )
        .await;
    }
}
