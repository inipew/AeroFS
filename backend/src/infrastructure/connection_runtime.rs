use crate::config::AppConfig;
use crate::domain::{Connection, ProviderKind, VfsPath};
use crate::errors::{AppError, VfsError};
use crate::ports::connections::{
    ConnectionEffects, ConnectionRuntime, ConnectionRuntimeInfo, DetachedConnectionRuntime,
    PreparedConnectionRuntime,
};
use crate::services::MetadataCache;
use crate::transfer::TransferManager;
use crate::vfs::factory::ProviderFactory;
use crate::vfs::registry::ProviderRegistry;
use crate::vfs::runtime::StorageRuntime;
use crate::vfs::FileSystem;
use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct RegistryConnectionRuntime {
    config: Arc<AppConfig>,
    registry: Arc<ProviderRegistry>,
}

impl RegistryConnectionRuntime {
    pub fn new(config: Arc<AppConfig>, registry: Arc<ProviderRegistry>) -> Self {
        Self { config, registry }
    }

    fn provider_name(provider: ProviderKind) -> &'static str {
        match provider {
            ProviderKind::Ftp => "ftp",
            ProviderKind::Ftps => "ftps",
            ProviderKind::Sftp => "sftp",
            ProviderKind::S3 => "s3",
            ProviderKind::Local => "local",
        }
    }
}

struct PreparedRegistryConnection {
    id: String,
    provider: Arc<dyn FileSystem>,
    registry: Arc<ProviderRegistry>,
}

#[async_trait]
impl PreparedConnectionRuntime for PreparedRegistryConnection {
    async fn activate(self: Box<Self>) {
        if let Some(existing) = self.registry.get_runtime(&self.id).await {
            existing.set_state(crate::vfs::ProviderState::Draining).await;
        }
        self.registry.register(self.id, self.provider).await;
    }
}

struct DetachedRegistryConnection {
    id: String,
    runtime: Arc<StorageRuntime>,
    error_message: Option<String>,
    registry: Arc<ProviderRegistry>,
}

#[async_trait]
impl DetachedConnectionRuntime for DetachedRegistryConnection {
    async fn restore(self: Box<Self>) {
        self.registry
            .register_runtime(self.id.clone(), self.runtime)
            .await;
        if let Some(error) = self.error_message {
            self.registry.set_connection_error(&self.id, &error).await;
        }
    }
}

#[async_trait]
impl ConnectionRuntime for RegistryConnectionRuntime {
    async fn validate_target(&self, host: Option<&str>, port: Option<u16>) -> Result<(), AppError> {
        crate::security::validate_network_target(
            self.config.security.allow_private_network_connections,
            host,
            port,
        )
        .map_err(|error| AppError::Forbidden(error.to_string()))?;
        if let Some(host) = host {
            crate::security::ssrf::validate_after_dns(
                self.config.security.allow_private_network_connections,
                host,
                port.unwrap_or(21),
            )
            .await
            .map_err(|error| AppError::Forbidden(error.to_string()))?;
        }
        Ok(())
    }

    async fn prepare_local(
        &self,
        root: &Path,
    ) -> Result<Box<dyn PreparedConnectionRuntime>, AppError> {
        tokio::fs::create_dir_all(root).await.map_err(|error| {
            AppError::Internal(anyhow::anyhow!(
                "Failed to create local root directory '{}': {}",
                root.display(),
                error
            ))
        })?;
        let config = self.config.storage.get_provider_config("local");
        let provider = ProviderFactory::build_local_with_config(
            "local",
            root.to_path_buf(),
            Some(&config),
        )
        .map_err(|error| AppError::Internal(anyhow::anyhow!(
            "Failed to initialize local storage provider: {}",
            error
        )))?;
        Ok(Box::new(PreparedRegistryConnection {
            id: "local".to_string(),
            provider,
            registry: self.registry.clone(),
        }))
    }

    async fn prepare(
        &self,
        connection: &Connection,
        secret: Option<&str>,
    ) -> Result<Box<dyn PreparedConnectionRuntime>, AppError> {
        let config = self
            .config
            .storage
            .get_provider_config(Self::provider_name(connection.provider));
        let provider = ProviderFactory::build_with_config(connection, secret, Some(&config))
            .map_err(|error| {
                AppError::BadRequest(format!("Failed to build provider: {}", error))
            })?;
        Ok(Box::new(PreparedRegistryConnection {
            id: connection.id.clone(),
            provider,
            registry: self.registry.clone(),
        }))
    }

    async fn set_error(&self, id: &str, error: &str) {
        self.registry.set_connection_error(id, error).await;
    }

    async fn info(&self, id: &str) -> ConnectionRuntimeInfo {
        let provider = self.registry.get(id).await;
        ConnectionRuntimeInfo {
            active: provider.is_some(),
            error_message: self.registry.get_connection_error(id).await,
            capabilities: provider.map(|provider| provider.capabilities()),
        }
    }

    async fn detach(&self, id: &str) -> Option<Box<dyn DetachedConnectionRuntime>> {
        let runtime = self.registry.get_runtime(id).await?;
        let error_message = self.registry.get_connection_error(id).await;
        self.registry.remove(id).await;
        Some(Box::new(DetachedRegistryConnection {
            id: id.to_string(),
            runtime,
            error_message,
            registry: self.registry.clone(),
        }))
    }

    async fn remove(&self, id: &str) {
        self.registry.remove(id).await;
    }

    async fn test(&self, id: &str) -> Result<u64, AppError> {
        let provider = self
            .registry
            .get(id)
            .await
            .ok_or_else(|| VfsError::NotFound(format!("Connection '{}' not found", id)))?;
        let root = VfsPath::root(id);
        let start = std::time::Instant::now();
        provider.stat(&root).await?;
        Ok(start.elapsed().as_millis() as u64)
    }
}

#[derive(Clone)]
pub struct RuntimeConnectionEffects {
    transfer_manager: TransferManager,
    metadata_cache: Arc<MetadataCache>,
}

impl RuntimeConnectionEffects {
    pub fn new(transfer_manager: TransferManager, metadata_cache: Arc<MetadataCache>) -> Self {
        Self {
            transfer_manager,
            metadata_cache,
        }
    }
}

#[async_trait]
impl ConnectionEffects for RuntimeConnectionEffects {
    async fn cancel_active_transfers(&self, id: &str) -> Result<(), AppError> {
        let active_jobs = self.transfer_manager.list_jobs(None, true, false).await;
        for job in active_jobs {
            if (job.source_connection_id == id || job.destination_connection_id == id)
                && job.status.is_active()
            {
                if let Err(error) = self.transfer_manager.cancel_job(&job.id, None, true).await {
                    let current = self.transfer_manager.get_job(&job.id).await;
                    if current.map(|job| job.status.is_terminal()).unwrap_or(true) {
                        continue;
                    }
                    return Err(AppError::Internal(anyhow::anyhow!(
                        "Failed to request cancellation for transfer '{}' before deleting connection '{}': {}",
                        job.id,
                        id,
                        error
                    )));
                }
            }
        }
        Ok(())
    }

    async fn invalidate_metadata(&self, id: &str) {
        self.metadata_cache.invalidate_prefix(id, "/").await;
    }
}
