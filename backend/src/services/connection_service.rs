use crate::config::AppConfig;
use crate::db::DbPool;
use crate::domain::{Actor, Capabilities, Connection, ConnectionStatus, ProviderKind, VfsPath};
use crate::errors::{AppError, VfsError};
use crate::infrastructure::CredentialStore;
use crate::services::MetadataCache;
use crate::transfer::{TransferManager, TransferStatus};
use crate::vfs::factory::ProviderFactory;
use crate::vfs::registry::ProviderRegistry;
use chrono::{DateTime, Utc};
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

type ConnectionDbRow = (
    String,
    String,
    String,
    Option<String>,
    Option<i64>,
    Option<String>,
    String,
    i64,
    i64,
    String,
    String,
);

#[derive(Clone)]
pub struct ConnectionService {
    db: DbPool,
    config: Arc<AppConfig>,
    registry: Arc<ProviderRegistry>,
    credentials: Arc<CredentialStore>,
    metadata_cache: Arc<MetadataCache>,
    transfer_manager: TransferManager,
}

impl ConnectionService {
    pub fn new(
        db: DbPool,
        config: Arc<AppConfig>,
        registry: Arc<ProviderRegistry>,
        credentials: Arc<CredentialStore>,
        metadata_cache: Arc<MetadataCache>,
        transfer_manager: TransferManager,
    ) -> Self {
        Self {
            db,
            config,
            registry,
            credentials,
            metadata_cache,
            transfer_manager,
        }
    }

    fn provider_kind(provider: &str) -> ProviderKind {
        match provider {
            "ftp" => ProviderKind::Ftp,
            "ftps" => ProviderKind::Ftps,
            "sftp" => ProviderKind::Sftp,
            "s3" => ProviderKind::S3,
            _ => ProviderKind::Local,
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
        if actor.is_admin {
            return Ok(());
        }
        let allowed: Option<(i64,)> = sqlx::query_as(
            "SELECT can_read FROM permissions WHERE user_id = ? AND connection_id = ? LIMIT 1",
        )
        .bind(&actor.id)
        .bind(id)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;
        if allowed.map(|r| r.0 != 0).unwrap_or(false) {
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
    pub async fn load_all_providers_from_db(&self) {
        let local_root = sqlx::query_as::<_, (String,)>(
            "SELECT value FROM system_settings WHERE key = 'local_root'",
        )
        .fetch_optional(&self.db)
        .await
        .ok()
        .flatten()
        .map(|r| std::path::PathBuf::from(r.0))
        .unwrap_or_else(|| self.config.filesystem.default_local_root.clone());

        if let Err(e) = tokio::fs::create_dir_all(&local_root).await {
            tracing::error!("Failed to create local root dir {:?}: {}", local_root, e);
        }
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

        type EnabledRow = (
            String,
            String,
            String,
            Option<String>,
            Option<i64>,
            Option<String>,
            String,
        );
        let rows: Vec<EnabledRow> = sqlx::query_as(
            "SELECT id, name, provider, host, port, username, base_path FROM connections WHERE enabled = 1",
        )
        .fetch_all(&self.db)
        .await
        .unwrap_or_default();

        for (id, name, provider_type, host, port, username, base_path) in rows {
            if id == "local" {
                continue;
            }
            let secret_row: Option<(String,)> = sqlx::query_as(
                "SELECT encrypted_secret FROM connection_credentials WHERE connection_id = ?",
            )
            .bind(&id)
            .fetch_optional(&self.db)
            .await
            .unwrap_or(None);
            let decrypted_secret = secret_row.and_then(|r| self.credentials.decrypt(&r.0).ok());
            let conn = Connection {
                id: id.clone(),
                name: name.clone(),
                provider: Self::provider_kind(&provider_type),
                host,
                port: port.map(|p| p as u16),
                username,
                base_path,
                read_only: false,
                enabled: true,
                status: ConnectionStatus::Connected,
                error_message: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };
            let provider_cfg = self.config.storage.get_provider_config(&provider_type);
            match ProviderFactory::build_with_config(
                &conn,
                decrypted_secret.as_deref(),
                Some(&provider_cfg),
            ) {
                Ok(fs) => {
                    self.registry.register(id.clone(), fs).await;
                    tracing::info!(
                        "Storage connection '{}' ('{}', {}) initialized successfully",
                        id,
                        name,
                        provider_type
                    );
                }
                Err(e) => {
                    let err_msg = e.to_string();
                    tracing::error!(
                        "Failed to initialize storage connection '{}' ('{}', {}): {}",
                        id,
                        name,
                        provider_type,
                        err_msg
                    );
                    self.registry.set_connection_error(&id, &err_msg).await;
                }
            }
        }
    }

    pub async fn list_connections(&self, actor: &Actor) -> Result<Vec<Connection>, AppError> {
        let rows: Vec<ConnectionDbRow> = if actor.is_admin {
            sqlx::query_as(
                "SELECT id, name, provider, host, port, username, base_path, read_only, enabled, created_at, updated_at FROM connections ORDER BY name ASC",
            )
            .fetch_all(&self.db)
            .await
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))?
        } else {
            sqlx::query_as(
                "SELECT c.id, c.name, c.provider, c.host, c.port, c.username, c.base_path, c.read_only, c.enabled, c.created_at, c.updated_at FROM connections c JOIN permissions p ON p.connection_id = c.id WHERE p.user_id = ? AND p.can_read = 1 ORDER BY c.name ASC",
            )
            .bind(&actor.id)
            .fetch_all(&self.db)
            .await
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))?
        };

        let mut connections = Vec::with_capacity(rows.len());
        for (
            id,
            name,
            provider,
            host,
            port,
            username,
            base_path,
            read_only,
            enabled,
            created_at,
            updated_at,
        ) in rows
        {
            let is_active = self.registry.get(&id).await.is_some();
            let error_message = self.registry.get_connection_error(&id).await;
            let status = if enabled == 0 {
                ConnectionStatus::Disconnected
            } else if is_active {
                ConnectionStatus::Connected
            } else {
                ConnectionStatus::Failed
            };
            connections.push(Connection {
                id,
                name,
                provider: Self::provider_kind(&provider),
                host,
                port: port.map(|p| p as u16),
                username,
                base_path,
                read_only: read_only != 0,
                enabled: enabled != 0,
                status,
                error_message,
                created_at: DateTime::parse_from_rfc3339(&created_at)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                updated_at: DateTime::parse_from_rfc3339(&updated_at)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            });
        }
        Ok(connections)
    }

    pub async fn get_connection(
        &self,
        actor: &Actor,
        id: &str,
    ) -> Result<ConnectionDetailResponse, AppError> {
        self.authorize_read(actor, id).await?;
        let provider =
            self.registry.get(id).await.ok_or_else(|| {
                VfsError::ConnectionError(format!("Connection '{}' not found", id))
            })?;
        let row: Option<ConnectionDbRow> = sqlx::query_as(
            "SELECT id, name, provider, host, port, username, base_path, read_only, enabled, created_at, updated_at FROM connections WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;
        let (
            id,
            name,
            provider_name,
            host,
            port,
            username,
            base_path,
            read_only,
            enabled,
            created_at,
            updated_at,
        ) = row.ok_or_else(|| VfsError::NotFound(format!("Connection '{}' not found", id)))?;
        let is_active = self.registry.get(&id).await.is_some();
        let error_message = self.registry.get_connection_error(&id).await;
        let status = if enabled == 0 {
            ConnectionStatus::Disconnected
        } else if is_active {
            ConnectionStatus::Connected
        } else {
            ConnectionStatus::Failed
        };
        Ok(ConnectionDetailResponse {
            connection: Connection {
                id,
                name,
                provider: Self::provider_kind(&provider_name),
                host,
                port: port.map(|p| p as u16),
                username,
                base_path,
                read_only: read_only != 0,
                enabled: enabled != 0,
                status,
                error_message,
                created_at: DateTime::parse_from_rfc3339(&created_at)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                updated_at: DateTime::parse_from_rfc3339(&updated_at)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            },
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
        let now = Utc::now().to_rfc3339();
        let provider_name = Self::provider_name(payload.provider);
        let base_path = payload.base_path.clone().unwrap_or_else(|| "/".to_string());
        let read_only = payload.read_only.unwrap_or(false);
        let conn = Connection {
            id: id.clone(),
            name: payload.name.clone(),
            provider: payload.provider,
            host: payload.host.clone(),
            port: payload.port,
            username: payload.username.clone(),
            base_path: base_path.clone(),
            read_only,
            enabled: true,
            status: ConnectionStatus::Connected,
            error_message: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let provider_cfg = self.config.storage.get_provider_config(provider_name);
        let fs = ProviderFactory::build_with_config(
            &conn,
            payload.secret.as_deref(),
            Some(&provider_cfg),
        )
        .map_err(|e| AppError::BadRequest(format!("Failed to build provider: {}", e)))?;

        let mut tx = self
            .db
            .begin()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to begin transaction: {}", e))?;
        sqlx::query(
            "INSERT INTO connections (id, name, provider, host, port, username, base_path, read_only, enabled, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?)",
        )
        .bind(&id)
        .bind(&payload.name)
        .bind(provider_name)
        .bind(&payload.host)
        .bind(payload.port.map(|p| p as i64))
        .bind(&payload.username)
        .bind(&base_path)
        .bind(if read_only { 1 } else { 0 })
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to save connection: {}", e))?;
        if let Some(secret) = &payload.secret {
            if !secret.trim().is_empty() {
                let encrypted = self.credentials.encrypt(secret)?;
                sqlx::query("INSERT INTO connection_credentials (connection_id, credential_type, encrypted_secret, created_at) VALUES (?, 'password_or_key', ?, ?)")
                    .bind(&id)
                    .bind(&encrypted)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to save credential: {}", e))?;
            }
        }
        tx.commit()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to commit connection transaction: {}", e))?;
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
        let row: Option<ConnectionDbRow> = sqlx::query_as(
            "SELECT id, name, provider, host, port, username, base_path, read_only, enabled, created_at, updated_at FROM connections WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;
        let (
            _,
            cur_name,
            provider_name,
            cur_host,
            cur_port,
            cur_username,
            cur_base_path,
            cur_read_only,
            cur_enabled,
            created_at,
            _,
        ) = row.ok_or_else(|| VfsError::NotFound(format!("Connection '{}' not found", id)))?;
        let new_name = payload.name.unwrap_or(cur_name);
        let new_host = payload.host.or(cur_host);
        let new_port = payload.port.or(cur_port.map(|p| p as u16));
        let new_username = payload.username.or(cur_username);
        let new_base_path = payload.base_path.unwrap_or(cur_base_path);
        let new_read_only = payload.read_only.unwrap_or(cur_read_only != 0);
        let new_enabled = payload.enabled.unwrap_or(cur_enabled != 0);
        self.validate_target(new_host.as_deref(), new_port).await?;
        let now = Utc::now().to_rfc3339();
        let updated_conn = Connection {
            id: id.to_string(),
            name: new_name.clone(),
            provider: Self::provider_kind(&provider_name),
            host: new_host.clone(),
            port: new_port,
            username: new_username.clone(),
            base_path: new_base_path.clone(),
            read_only: new_read_only,
            enabled: new_enabled,
            status: if new_enabled {
                ConnectionStatus::Connected
            } else {
                ConnectionStatus::Disconnected
            },
            error_message: None,
            created_at: DateTime::parse_from_rfc3339(&created_at)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: Utc::now(),
        };
        let resolved_secret = if let Some(secret) = payload.secret {
            if secret.trim().is_empty() {
                None
            } else {
                Some(secret)
            }
        } else {
            let row: Option<(String,)> = sqlx::query_as(
                "SELECT encrypted_secret FROM connection_credentials WHERE connection_id = ?",
            )
            .bind(id)
            .fetch_optional(&self.db)
            .await
            .unwrap_or(None);
            row.and_then(|(encrypted,)| self.credentials.decrypt(&encrypted).ok())
        };

        if new_enabled {
            let provider_cfg = self.config.storage.get_provider_config(&provider_name);
            let fs = ProviderFactory::build_with_config(
                &updated_conn,
                resolved_secret.as_deref(),
                Some(&provider_cfg),
            )
            .map_err(|e| {
                AppError::BadRequest(format!("Failed to build updated provider: {}", e))
            })?;
            let mut tx = self
                .db
                .begin()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to begin transaction: {}", e))?;
            sqlx::query("UPDATE connections SET name = ?, host = ?, port = ?, username = ?, base_path = ?, read_only = ?, enabled = ?, updated_at = ? WHERE id = ?")
                .bind(&new_name)
                .bind(&new_host)
                .bind(new_port.map(|p| p as i64))
                .bind(&new_username)
                .bind(&new_base_path)
                .bind(if new_read_only { 1 } else { 0 })
                .bind(if new_enabled { 1 } else { 0 })
                .bind(&now)
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to update connection in DB: {}", e))?;
            if let Some(secret) = &resolved_secret {
                let encrypted = self.credentials.encrypt(secret)?;
                sqlx::query("INSERT INTO connection_credentials (connection_id, credential_type, encrypted_secret, created_at) VALUES (?, 'password_or_key', ?, ?) ON CONFLICT(connection_id) DO UPDATE SET encrypted_secret = excluded.encrypted_secret, created_at = excluded.created_at")
                    .bind(id)
                    .bind(&encrypted)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to save credential: {}", e))?;
            }
            tx.commit()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to commit update transaction: {}", e))?;
            if let Some(existing) = self.registry.get_runtime(id).await {
                existing
                    .set_state(crate::vfs::ProviderState::Draining)
                    .await;
            }
            self.registry.register(id.to_string(), fs).await;
        } else {
            sqlx::query("UPDATE connections SET enabled = 0, updated_at = ? WHERE id = ?")
                .bind(&now)
                .bind(id)
                .execute(&self.db)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to disable connection: {}", e))?;
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
        let mut tx = self
            .db
            .begin()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to begin transaction: {}", e))?;
        sqlx::query("DELETE FROM connection_credentials WHERE connection_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to delete connection credential: {}", e))?;
        let result = sqlx::query("DELETE FROM connections WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to delete connection: {}", e))?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("Connection '{}' not found", id)));
        }
        tx.commit()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to commit connection deletion: {}", e))?;

        let active_jobs = self.transfer_manager.list_jobs(None, true, false).await;
        for job in active_jobs {
            if (job.source_connection_id == id || job.destination_connection_id == id)
                && matches!(
                    job.status,
                    TransferStatus::Running
                        | TransferStatus::Queued
                        | TransferStatus::CancellationRequested
                )
            {
                let _ = self.transfer_manager.cancel_job(&job.id, None, true).await;
            }
        }
        self.registry.remove(id).await;
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
