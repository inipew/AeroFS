use crate::auth::permissions::{check_permission, PermissionAction};
use crate::auth::AuthenticatedUser;
use crate::domain::{Capabilities, Connection, ConnectionStatus, ProviderKind};
use crate::errors::{AppError, VfsError};
use crate::state::AppState;
use crate::vfs::factory::ProviderFactory;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
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

pub struct ConnectionService;

impl ConnectionService {
    /// Load all enabled storage connections from SQLite and register into ProviderRegistry
    pub async fn load_all_providers_from_db(state: &AppState) {
        // 1. Always load local filesystem provider
        let local_root = if let Some(custom) =
            crate::services::SettingsService::get_system_setting(state, "local_root").await
        {
            std::path::PathBuf::from(custom)
        } else {
            state.config.filesystem.default_local_root.clone()
        };
        if let Err(e) = tokio::fs::create_dir_all(&local_root).await {
            tracing::error!("Failed to create local root dir {:?}: {}", local_root, e);
        }
        let local_cfg = state.config.storage.get_provider_config("local");
        match ProviderFactory::build_local_with_config("local", local_root.clone(), Some(&local_cfg)) {
            Ok(local_fs) => {
                state.registry.register("local".to_string(), local_fs).await;
                tracing::info!("Default Local Storage provider loaded at {:?}", local_root);
            }
            Err(e) => {
                tracing::error!("Failed to init Local Storage provider: {}", e);
                state
                    .registry
                    .set_connection_error("local", &e.to_string())
                    .await;
            }
        }

        // 2. Query database for other enabled connections
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
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

        for (id, name, provider_type, host, port, username, base_path) in rows {
            if id == "local" {
                continue;
            }

            // Retrieve encrypted credential if exists
            let secret_row: Option<(String,)> = sqlx::query_as(
                "SELECT encrypted_secret FROM connection_credentials WHERE connection_id = ?",
            )
            .bind(&id)
            .fetch_optional(&state.db)
            .await
            .unwrap_or(None);

            let decrypted_secret = secret_row.and_then(|r| state.credentials.decrypt(&r.0).ok());

            let provider_kind = match provider_type.as_str() {
                "ftp" => ProviderKind::Ftp,
                "ftps" => ProviderKind::Ftps,
                "sftp" => ProviderKind::Sftp,
                "s3" => ProviderKind::S3,
                _ => ProviderKind::Local,
            };

            let conn = Connection {
                id: id.clone(),
                name: name.clone(),
                provider: provider_kind,
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
            let provider_cfg = state.config.storage.get_provider_config(&provider_type);
            match ProviderFactory::build_with_config(&conn, decrypted_secret.as_deref(), Some(&provider_cfg)) {
                Ok(fs) => {
                    state.registry.register(id.clone(), fs).await;
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
                    state.registry.set_connection_error(&id, &err_msg).await;
                }
            }
        }
    }

    /// List all connections accessible to the user
    pub async fn list_connections(
        state: &AppState,
        user: &AuthenticatedUser,
    ) -> Result<Vec<Connection>, AppError> {
        let rows: Vec<ConnectionDbRow> = if user.is_admin {
            sqlx::query_as(
                "SELECT id, name, provider, host, port, username, base_path, read_only, enabled, created_at, updated_at 
                 FROM connections ORDER BY name ASC",
            )
            .fetch_all(&state.db)
            .await
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))?
        } else {
            sqlx::query_as(
                "SELECT c.id, c.name, c.provider, c.host, c.port, c.username, c.base_path, c.read_only, c.enabled, c.created_at, c.updated_at 
                 FROM connections c
                 JOIN permissions p ON p.connection_id = c.id
                 WHERE p.user_id = ? AND p.can_read = 1
                 ORDER BY c.name ASC",
            )
            .bind(&user.id)
            .fetch_all(&state.db)
            .await
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))?
        };

        let mut connections = Vec::new();
        for (
            id,
            name,
            provider_str,
            host,
            port,
            username,
            base_path,
            read_only,
            enabled,
            created_at_str,
            updated_at_str,
        ) in rows
        {
            let provider = match provider_str.as_str() {
                "ftp" => ProviderKind::Ftp,
                "ftps" => ProviderKind::Ftps,
                "sftp" => ProviderKind::Sftp,
                "s3" => ProviderKind::S3,
                _ => ProviderKind::Local,
            };

            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            let is_active = state.registry.get(&id).await.is_some();
            let error_message = state.registry.get_connection_error(&id).await;
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
                provider,
                host,
                port: port.map(|p| p as u16),
                username,
                base_path,
                read_only: read_only != 0,
                enabled: enabled != 0,
                status,
                error_message,
                created_at,
                updated_at,
            });
        }

        Ok(connections)
    }

    /// Get connection detail along with operational capabilities
    pub async fn get_connection(
        state: &AppState,
        user: &AuthenticatedUser,
        id: &str,
    ) -> Result<ConnectionDetailResponse, AppError> {
        check_permission(&state.db, user, id, PermissionAction::Read).await?;

        let provider =
            state.registry.get(id).await.ok_or_else(|| {
                VfsError::ConnectionError(format!("Connection '{}' not found", id))
            })?;

        let row: Option<ConnectionDbRow> = sqlx::query_as(
            "SELECT id, name, provider, host, port, username, base_path, read_only, enabled, created_at, updated_at 
             FROM connections WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

        let (
            id,
            name,
            provider_str,
            host,
            port,
            username,
            base_path,
            read_only,
            enabled,
            created_at_str,
            updated_at_str,
        ) = row.ok_or_else(|| VfsError::NotFound(format!("Connection '{}' not found", id)))?;

        let provider_kind = match provider_str.as_str() {
            "ftp" => ProviderKind::Ftp,
            "ftps" => ProviderKind::Ftps,
            "sftp" => ProviderKind::Sftp,
            "s3" => ProviderKind::S3,
            _ => ProviderKind::Local,
        };

        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        let is_active = state.registry.get(&id).await.is_some();
        let error_message = state.registry.get_connection_error(&id).await;
        let status = if enabled == 0 {
            ConnectionStatus::Disconnected
        } else if is_active {
            ConnectionStatus::Connected
        } else {
            ConnectionStatus::Failed
        };

        let connection = Connection {
            id,
            name,
            provider: provider_kind,
            host,
            port: port.map(|p| p as u16),
            username,
            base_path,
            read_only: read_only != 0,
            enabled: enabled != 0,
            status,
            error_message,
            created_at,
            updated_at,
        };

        Ok(ConnectionDetailResponse {
            connection,
            capabilities: provider.capabilities(),
        })
    }

    /// Create new connection (admin only) with credential encryption and provider registration
    pub async fn create_connection(
        state: &AppState,
        user: &AuthenticatedUser,
        payload: CreateConnectionRequest,
    ) -> Result<String, AppError> {
        if !user.is_admin {
            return Err(AppError::Forbidden(
                "Only administrators can create storage connections".into(),
            ));
        }

        let id = format!("conn_{}", &Uuid::new_v4().to_string()[..8]);
        let now = Utc::now().to_rfc3339();
        let provider_str = match payload.provider {
            ProviderKind::Ftp => "ftp",
            ProviderKind::Ftps => "ftps",
            ProviderKind::Sftp => "sftp",
            ProviderKind::S3 => "s3",
            ProviderKind::Local => "local",
        };

        let base_path = payload.base_path.unwrap_or_else(|| "/".to_string());
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

        // Build live provider FIRST before saving (fail-closed)
        let provider_cfg = state.config.storage.get_provider_config(provider_str);
        let fs = ProviderFactory::build_with_config(&conn, payload.secret.as_deref(), Some(&provider_cfg))
            .map_err(|e| AppError::BadRequest(format!("Failed to build provider: {}", e)))?;

        // Save connection to DB in transaction
        let mut tx = state
            .db
            .begin()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to begin transaction: {}", e))?;

        sqlx::query(
            "INSERT INTO connections (id, name, provider, host, port, username, base_path, read_only, enabled, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?)"
        )
        .bind(&id)
        .bind(&payload.name)
        .bind(provider_str)
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

        // Encrypt and persist credentials
        if let Some(secret) = &payload.secret {
            if !secret.trim().is_empty() {
                let encrypted = state.credentials.encrypt(secret)?;
                sqlx::query(
                    "INSERT INTO connection_credentials (connection_id, credential_type, encrypted_secret, created_at)
                     VALUES (?, 'password_or_key', ?, ?)"
                )
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

        state.registry.register(id.clone(), fs).await;

        Ok(id)
    }

    /// Delete connection (admin only)
    pub async fn delete_connection(
        state: &AppState,
        user: &AuthenticatedUser,
        id: &str,
    ) -> Result<(), AppError> {
        if !user.is_admin {
            return Err(AppError::Forbidden(
                "Access forbidden: Only administrators can delete storage connections".to_string(),
            ));
        }

        if id == "local" {
            return Err(AppError::BadRequest(
                "Default local connection cannot be deleted".into(),
            ));
        }

        let _ = sqlx::query("DELETE FROM connection_credentials WHERE connection_id = ?")
            .bind(id)
            .execute(&state.db)
            .await;

        let res = sqlx::query("DELETE FROM connections WHERE id = ?")
            .bind(id)
            .execute(&state.db)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to delete connection: {}", e))?;

        if res.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("Connection '{}' not found", id)));
        }

        // Cancel all active transfers using this connection (Plan 39 P1.18)
        let active_jobs = state.transfer_manager.list_jobs(None, true, false).await;
        for job in active_jobs {
            if (job.source_connection_id == id || job.destination_connection_id == id)
                && (job.status == crate::transfer::TransferStatus::Running
                    || job.status == crate::transfer::TransferStatus::Queued
                    || job.status == crate::transfer::TransferStatus::CancellationRequested)
            {
                let _ = state.transfer_manager.cancel_job(&job.id, None, true).await;
            }
        }

        state.registry.remove(id).await;

        Ok(())
    }

    /// Test connectivity
    pub async fn test_connection(
        state: &AppState,
        user: &AuthenticatedUser,
        id: &str,
    ) -> Result<TestConnectionResponse, AppError> {
        if id == "local" {
            return Ok(TestConnectionResponse {
                success: true,
                latency_ms: 0,
                message: "Local filesystem connected".to_string(),
            });
        }

        check_permission(&state.db, user, id, PermissionAction::Read).await?;

        let provider = state
            .registry
            .get(id)
            .await
            .ok_or_else(|| VfsError::NotFound(format!("Connection '{}' not found", id)))?;

        let root_path = crate::domain::VfsPath::root(id);
        let start = std::time::Instant::now();
        provider.stat(&root_path).await?;
        let latency = start.elapsed().as_millis() as u64;

        Ok(TestConnectionResponse {
            success: true,
            latency_ms: latency,
            message: format!("Connection '{}' verified ({} ms)", id, latency),
        })
    }
}
