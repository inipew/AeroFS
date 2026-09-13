use crate::domain::{Actor, Capabilities, Connection, ConnectionStatus, ProviderKind};
use crate::errors::{AppError, VfsError};
use crate::ports::{
    connections::{
        ConnectionEffects, ConnectionRepository, ConnectionRuntime, ConnectionSecretError,
        SecretMutation,
    },
    settings::FileSettings,
};
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
    settings: Arc<dyn FileSettings>,
    runtime: Arc<dyn ConnectionRuntime>,
    effects: Arc<dyn ConnectionEffects>,
}

impl ConnectionService {
    pub fn new(
        repository: Arc<dyn ConnectionRepository>,
        settings: Arc<dyn FileSettings>,
        runtime: Arc<dyn ConnectionRuntime>,
        effects: Arc<dyn ConnectionEffects>,
    ) -> Self {
        Self {
            repository,
            settings,
            runtime,
            effects,
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

    /// Load all enabled storage connections and publish their prepared runtime
    /// providers. Durable-state read failures are startup-fatal; individual
    /// provider/decryption failures are isolated to the affected connection.
    pub async fn load_all_providers_from_db(&self) -> Result<(), AppError> {
        let local_root = self.settings.local_root().await?;
        match self.runtime.prepare_local(&local_root).await {
            Ok(prepared) => prepared.activate().await,
            Err(error) => {
                let message = error.to_string();
                tracing::error!(%message, "failed to initialize local storage runtime");
                self.runtime.set_error("local", &message).await;
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
                    self.runtime.set_error(&connection.id, &message).await;
                    continue;
                }
            };

            match self
                .runtime
                .prepare(&connection, decrypted_secret.as_deref())
                .await
            {
                Ok(prepared) => {
                    prepared.activate().await;
                    tracing::info!(
                        connection_id = %connection.id,
                        connection_name = %connection.name,
                        "storage connection initialized successfully"
                    );
                }
                Err(error) => {
                    let message = error.to_string();
                    tracing::error!(connection_id = %connection.id, %message, "failed to initialize storage connection");
                    self.runtime.set_error(&connection.id, &message).await;
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
            let info = self.runtime.info(&connection.id).await;
            connection.error_message = info.error_message;
            connection.status = if !connection.enabled {
                ConnectionStatus::Disconnected
            } else if info.active {
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
        let mut connection = self
            .repository
            .get(id)
            .await?
            .ok_or_else(|| VfsError::NotFound(format!("Connection '{}' not found", id)))?;
        let info = self.runtime.info(id).await;
        let capabilities = info.capabilities.ok_or_else(|| {
            VfsError::ConnectionError(format!("Connection '{}' not found", id))
        })?;
        connection.error_message = info.error_message;
        connection.status = if !connection.enabled {
            ConnectionStatus::Disconnected
        } else if info.active {
            ConnectionStatus::Connected
        } else {
            ConnectionStatus::Failed
        };
        Ok(ConnectionDetailResponse {
            connection,
            capabilities,
        })
    }

    pub async fn create_connection(
        &self,
        actor: &Actor,
        payload: CreateConnectionRequest,
    ) -> Result<String, AppError> {
        Self::require_admin(actor, "create")?;
        self.runtime
            .validate_target(payload.host.as_deref(), payload.port)
            .await?;

        let id = format!("conn_{}", &Uuid::new_v4().to_string()[..8]);
        let now = Utc::now();
        let connection = Connection {
            id: id.clone(),
            name: payload.name,
            provider: payload.provider,
            host: payload.host,
            port: payload.port,
            username: payload.username,
            base_path: payload.base_path.unwrap_or_else(|| "/".to_string()),
            read_only: payload.read_only.unwrap_or(false),
            enabled: true,
            status: ConnectionStatus::Connected,
            error_message: None,
            created_at: now,
            updated_at: now,
        };

        let prepared = self
            .runtime
            .prepare(&connection, payload.secret.as_deref())
            .await?;
        self.repository
            .create(&connection, payload.secret.as_deref())
            .await?;
        prepared.activate().await;
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
        self.runtime
            .validate_target(new_host.as_deref(), new_port)
            .await?;

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

        let prepared = if new_enabled {
            Some(
                self.runtime
                    .prepare(&updated_connection, resolved_secret.as_deref())
                    .await?,
            )
        } else {
            None
        };

        self.repository
            .update(&updated_connection, secret_mutation)
            .await?;

        if let Some(prepared) = prepared {
            prepared.activate().await;
        } else {
            self.runtime.remove(id).await;
        }
        self.effects.invalidate_metadata(id).await;
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

        let detached = self.runtime.detach(id).await;
        let deletion_result: Result<(), AppError> = async {
            self.effects.cancel_active_transfers(id).await?;
            if !self.repository.delete_with_transfer_barrier(id).await? {
                return Err(AppError::NotFound(format!("Connection '{}' not found", id)));
            }
            Ok(())
        }
        .await;

        if let Err(error) = deletion_result {
            if let Some(detached) = detached {
                detached.restore().await;
            }
            return Err(error);
        }

        self.effects.invalidate_metadata(id).await;
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
        let latency_ms = self.runtime.test(id).await?;
        Ok(TestConnectionResponse {
            success: true,
            latency_ms,
            message: format!("Connection '{}' verified ({} ms)", id, latency_ms),
        })
    }
}
