use crate::config::AppConfig;
use crate::domain::{Actor, Capabilities, Connection, ConnectionStatus, ProviderKind, VfsPath};
use crate::errors::{AppError, VfsError};
use crate::ports::connections::{
    ConnectionRepository, ConnectionSecretError, SecretMutation,
};
use crate::services::MetadataCache;
use crate::transfer::{TransferManager, TransferStatus};
use crate::vfs::factory::ProviderFactory;
use crate::vfs::registry::ProviderRegistry;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateConnectionRequest {
    pub name: String,
    pub provider: ProviderKind,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub secret: Option<String>,
    pub base_path: Option<String>,
    pub read_only: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateConnectionRequest {
    pub name: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub secret: Option<String>,
    pub base_path: Option<String>,
    pub read_only: Option<bool>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ConnectionDetailResponse {
    pub connection: Connection,
    pub capabilities: Capabilities,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TestConnectionResponse {
    pub success: bool,
    pub latency_ms: u64,
    pub message: String,
}

#[derive(Clone)]
pub struct ConnectionService {
    repository: Arc<dyn ConnectionRepository>,
    config: Arc<AppConfig>,
    registry: Arc<ProviderRegistry>,
    metadata_cache: Arc<MetadataCache>,
    transfer_manager: TransferManager,
}

impl ConnectionService {
    pub fn new(
        repository: Arc<dyn ConnectionRepository>,
        config: Arc<AppConfig>,
        registry: Arc<ProviderRegistry>,
        metadata_cache: Arc<MetadataCache>,
        transfer_manager: TransferManager,
    ) -> Self {
        Self {
            repository,
            config,
            registry,
            metadata_cache,
            transfer_manager,
        }
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

    async fn authorize_read(&self, actor: &Actor, id: &str) -> Result<(), AppError> {
        if actor.is_admin || self.repository.can_read(&actor.id, id).await? {
            Ok(())
        } else {
            Err(AppError::Forbidden(format!(
                "Read access denied for connection '{}'",
                id
            )))
        }
    }

    fn require_admin(actor: &Actor, action: &str) -> Result<(), AppError> {
        if actor.is_admin {
            Ok(())
        } else {
            Err(AppError::Forbidden(format!(
                "Only administrators can {} storage connections",
                action
            )))
        }
    }

    async fn validate_target(&self, host: Option<&str>, port: Option<u16>) -> Result<(), AppError> {
        crate::security::validate_network_target(
            self.config.security.allow_private_network_connections,
            host,
            port,
        )
        .map_err(|e| AppError::Forbidden(e.to_string()))?;
        if let Some(host) = host {
            crate::security::ssrf::validate_after_dns(
                self.config.security.allow_private_network_connections,
                host,
                port.unwrap_or(21),
            )
            .await
            .map_err(|e| AppError::Forbidden(e.to_string()))?;
        }
        Ok(())
    }

    /// Load all enabled storage connections and register their providers.
    /// Durable-state read failures are startup-fatal. A credential that exists
    /// but cannot be decrypted is isolated to its connection so unrelated
    /// providers can still become available.
    pub async fn load_all_providers_from_db(&self) -> Result<(), AppError> {
        let local_root = self
            .repository
            .local_root_override()
            .await?
            .unwrap_or_else(|| self.config.filesystem.default_local_root.clone());

        match tokio::fs::create_dir_all(&local_root).await {
            Ok(()) => {
                let local_cfg = self.config.storage.get_provider_config("local");
                match ProviderFactory::build_local_with_config(
                    "local",
                    local_root.clone(),
                    Some(&local_cfg),
                ) {
                    Ok(local_fs) => {
                        self.registry.register("local".to_string(), local_fs).await;
                        tracing::info!("Default Local Storage provider loaded at {:?}", local_root);
                    }
                    Err(e) => {
                        tracing::error!("Failed to init Local Storage provider: {}", e);
                        self.registry
                            .set_connection_error("local", &e.to_string())
                            .await;
                    }
                }
            }
            Err(e) => {
                let error = format!(
                    "Failed to create local root directory '{}': {}",
                    local_root.display(),
                    e
                );
                tracing::error!(%error);
                self.registry.set_connection_error("local", &error).await;
            }
        }

        for connection in self.repository.load_enabled().await? {
            if connection.id == "local" {
                continue;
            }
            let decrypted_secret = match self.repository.load_secret(&connection.id).await {
                Ok(secret) => secret,
                Err(ConnectionSecretError::Persistence(error)) => {
                    return Err(AppError::Internal(anyhow::anyhow!(
                        "Failed to load credential for storage connection '{}': {}",
                        connection.id,
                        error
                    )));
                }
                Err(ConnectionSecretError::Decryption(error)) => {
                    let message = format!("Failed to decrypt persisted credential: {}", error);
                    tracing::error!(connection_id = %connection.id, %message);
                    self.registry
                        .set_connection_error(&connection.id, &message)
                        .await;
                    continue;
                }
            };
            let provider_name = Self::provider_name(connection.provider);
            let provider_cfg = self.config.storage.get_provider_config(provider_name);
            match ProviderFactory::build_with_config(
                &connection,
                decrypted_secret.as_deref(),
                Some(&provider_cfg),
            ) {
                Ok(fs) => {
                    self.registry.register(connection.id.clone(), fs).await;
                    tracing::info!(
                        "Storage connection '{}' ('{}', {}) initialized successfully",
                        connection.id,
                        connection.name,
                        provider_name
                    );
                }
                Err(e) => {
                    let err_msg = e.to_string();
                    tracing::error!(
                        "Failed to initialize storage connection '{}' ('{}', {}): {}",
                        connection.id,
                        connection.name,
                        provider_name,
                        err_msg
                    );
                    self.registry
                        .set_connection_error(&connection.id, &err_msg)
                        .await;
                }
            }
        }
        Ok(())
    }

    pub async fn list_connections(&self, actor: &Actor) -> Result<Vec<Connection>, AppError> {
        let mut connections = self
            .repository
            .list(Some(&actor.id), actor.is_admin)
            .await?;
        for connection in &mut connections {
            let is_active = self.registry.get(&connection.id).await.is_some();
            connection.error_message = self.registry.get_connection_error(&connection.id).await;
            connection.status = if !connection.enabled {
                ConnectionStatus::Disconnected
            } else if is_active {
                ConnectionStatus::Connected
            } else {
                ConnectionStatus::Failed
            };
        }
        Ok(connections)
    }

    pub async fn get_connection(
        &self,
        actor: &Actor,
        id: &str,
    ) -> Result<ConnectionDetailResponse, AppError> {
        self.authorize_read(actor, id).await?;
        let provider = self
            .registry
            .get(id)
            .await
            .ok_or_else(|| VfsError::ConnectionError(format!("Connection '{}' not found", id)))?;
        let mut connection = self
            .repository
            .get(id)
            .await?
            .ok_or_else(|| VfsError::NotFound(format!("Connection '{}' not found", id)))?;
        let is_active = self.registry.get(id).await.is_some();
        connection.error_message = self.registry.get_connection_error(id).await;
        connection.status = if !connection.enabled {
            ConnectionStatus::Disconnected
        } else if is_active {
            ConnectionStatus::Connected
        } else {
            ConnectionStatus::Failed
        };
        Ok(ConnectionDetailResponse {
            connection,
            capabilities: provider.capabilities(),
        })
    }

    pub async fn create_connection(
        &self,
        actor: &Actor,
        payload: CreateConnectionRequest,
    ) -> Result<String, AppError> {
        Self::require_admin(actor, "create")?;
        self.validate_target(payload.host.as_deref(), payload.port)
            .await?;

        let id = format!("conn_{}", &Uuid::new_v4().to_string()[..8]);
        let now = Utc::now();
        let base_path = payload.base_path.clone().unwrap_or_else(|| "/".to_string());
        let read_only = payload.read_only.unwrap_or(false);
        let connection = Connection {
            id: id.clone(),
            name: payload.name,
            provider: payload.provider,
            host: payload.host,
            port: payload.port,
            username: payload.username,
            base_path,
            read_only,
            enabled: true,
            status: ConnectionStatus::Connected,
            error_message: None,
            created_at: now,
            updated_at: now,
        };
        let provider_cfg = self
            .config
            .storage
            .get_provider_config(Self::provider_name(connection.provider));
        let fs = ProviderFactory::build_with_config(
            &connection,
            payload.secret.as_deref(),
            Some(&provider_cfg),
        )
        .map_err(|e| AppError::BadRequest(format!("Failed to build provider: {}", e)))?;

        self.repository
            .create(&connection, payload.secret.as_deref())
            .await?;
        self.registry.register(id.clone(), fs).await;
        Ok(id)
    }

    pub async fn update_connection(
        &self,
        actor: &Actor,
        id: &str,
        payload: UpdateConnectionRequest,
    ) -> Result<(), AppError> {
        Self::require_admin(actor, "update")?;
        if id == "local" {
            return Err(AppError::BadRequest(
                "Default local connection cannot be edited directly".into(),
            ));
        }
        let current = self
            .repository
            .get(id)
            .await?
            .ok_or_else(|| VfsError::NotFound(format!("Connection '{}' not found", id)))?;

        let new_name = payload.name.unwrap_or(current.name);
        let new_host = payload.host.or(current.host);
        let new_port = payload.port.or(current.port);
        let new_username = payload.username.or(current.username);
        let new_base_path = payload.base_path.unwrap_or(current.base_path);
        let new_read_only = payload.read_only.unwrap_or(current.read_only);
        let new_enabled = payload.enabled.unwrap_or(current.enabled);
        self.validate_target(new_host.as_deref(), new_port).await?;

        let (resolved_secret, secret_mutation) = match payload.secret {
            Some(secret) if secret.trim().is_empty() => (None, SecretMutation::Clear),
            Some(secret) => (Some(secret.clone()), SecretMutation::Replace(secret)),
            None => {
                let secret = self.repository.load_secret(id).await.map_err(|error| {
                    AppError::Internal(anyhow::anyhow!(
                        "Failed to load existing credential for '{}': {}",
                        id,
                        error
                    ))
                })?;
                (secret, SecretMutation::Keep)
            }
        };

        let updated_connection = Connection {
            id: id.to_string(),
            name: new_name,
            provider: current.provider,
            host: new_host,
            port: new_port,
            username: new_username,
            base_path: new_base_path,
            read_only: new_read_only,
            enabled: new_enabled,
            status: if new_enabled {
                ConnectionStatus::Connected
            } else {
                ConnectionStatus::Disconnected
            },
            error_message: None,
            created_at: current.created_at,
            updated_at: Utc::now(),
        };

        let prepared_provider = if new_enabled {
            let provider_cfg = self
                .config
                .storage
                .get_provider_config(Self::provider_name(updated_connection.provider));
            Some(
                ProviderFactory::build_with_config(
                    &updated_connection,
                    resolved_secret.as_deref(),
                    Some(&provider_cfg),
                )
                .map_err(|e| {
                    AppError::BadRequest(format!("Failed to build updated provider: {}", e))
                })?,
            )
        } else {
            None
        };

        self.repository
            .update(&updated_connection, secret_mutation)
            .await?;

        if let Some(fs) = prepared_provider {
            if let Some(existing) = self.registry.get_runtime(id).await {
                existing
                    .set_state(crate::vfs::ProviderState::Draining)
                    .await;
            }
            self.registry.register(id.to_string(), fs).await;
        } else {
            self.registry.remove(id).await;
        }
        self.metadata_cache.invalidate_prefix(id, "/").await;
        Ok(())
    }

    pub async fn delete_connection(&self, actor: &Actor, id: &str) -> Result<(), AppError> {
        Self::require_admin(actor, "delete")?;
        if id == "local" {
            return Err(AppError::BadRequest(
                "Default local connection cannot be deleted".into(),
            ));
        }
        if !self.repository.exists(id).await? {
            return Err(AppError::NotFound(format!("Connection '{}' not found", id)));
        }

        let previous_runtime = self.registry.get_runtime(id).await;
        self.registry.remove(id).await;

        let deletion_result: Result<(), AppError> = async {
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

            if !self.repository.delete_with_transfer_barrier(id).await? {
                return Err(AppError::NotFound(format!("Connection '{}' not found", id)));
            }
            Ok(())
        }
        .await;

        if let Err(error) = deletion_result {
            if let Some(runtime) = previous_runtime {
                self.registry.register_runtime(id.to_string(), runtime).await;
            }
            return Err(error);
        }

        self.metadata_cache.invalidate_prefix(id, "/").await;
        Ok(())
    }

    pub async fn test_connection(
        &self,
        actor: &Actor,
        id: &str,
    ) -> Result<TestConnectionResponse, AppError> {
        if id == "local" {
            return Ok(TestConnectionResponse {
                success: true,
                latency_ms: 0,
                message: "Local filesystem connected".to_string(),
            });
        }
        self.authorize_read(actor, id).await?;
        let provider = self
            .registry
            .get(id)
            .await
            .ok_or_else(|| VfsError::NotFound(format!("Connection '{}' not found", id)))?;
        let root = VfsPath::root(id);
        let start = std::time::Instant::now();
        provider.stat(&root).await?;
        let latency_ms = start.elapsed().as_millis() as u64;
        Ok(TestConnectionResponse {
            success: true,
            latency_ms,
            message: format!("Connection '{}' verified ({} ms)", id, latency_ms),
        })
    }
}
