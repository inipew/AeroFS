use crate::auth::credentials::{derive_master_key, encrypt_secret};
use crate::auth::AuthenticatedUser;
use crate::domain::{Capabilities, Connection, ConnectionStatus, ProviderKind};
use crate::errors::{AppError, VfsError};
use crate::state::AppState;
use crate::vfs::{FtpFileSystem, SftpFileSystem};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
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
    pub secret: Option<String>, // password or private key
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

/// List all available connections from database
pub async fn list_connections(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    let rows: Vec<(
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
    )> = sqlx::query_as(
        "SELECT id, name, provider, host, port, username, base_path, read_only, enabled, created_at, updated_at 
         FROM connections WHERE enabled = 1",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

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
            _ => ProviderKind::Local,
        };

        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

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
            status: ConnectionStatus::Connected,
            created_at,
            updated_at,
        });
    }

    Ok(Json(connections))
}

/// Create a new connection with encrypted credential storage
pub async fn create_connection(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Json(payload): Json<CreateConnectionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let id = format!("conn_{}", &Uuid::new_v4().to_string()[..8]);
    let now = Utc::now().to_rfc3339();
    let provider_str = match payload.provider {
        ProviderKind::Ftp => "ftp",
        ProviderKind::Ftps => "ftps",
        ProviderKind::Sftp => "sftp",
        ProviderKind::Local => "local",
    };

    let base_path = payload.base_path.unwrap_or_else(|| "/".to_string());
    let read_only = payload.read_only.unwrap_or(false);

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
    .execute(&state.db)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to save connection: {}", e))?;

    // Encrypt and save secret if provided
    let master_key = derive_master_key(&state.config.security.session_secret);
    if let Some(secret) = &payload.secret {
        if !secret.trim().is_empty() {
            let encrypted = encrypt_secret(&master_key, secret)?;
            sqlx::query(
                "INSERT INTO connection_credentials (connection_id, credential_type, encrypted_secret, created_at)
                 VALUES (?, 'password_or_key', ?, ?)"
            )
            .bind(&id)
            .bind(&encrypted)
            .bind(&now)
            .execute(&state.db)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to save credential: {}", e))?;
        }
    }

    // Register live provider in state
    match payload.provider {
        ProviderKind::Ftp => {
            let ftp_fs = Arc::new(FtpFileSystem::new(
                &id,
                payload.host.clone().unwrap_or_default(),
                payload.port.unwrap_or(21),
                payload.username.clone(),
                payload.secret.clone(),
                false,
                base_path.clone(),
            ));
            state.register_provider(id.clone(), ftp_fs).await;
        }
        ProviderKind::Ftps => {
            let ftps_fs = Arc::new(FtpFileSystem::new(
                &id,
                payload.host.clone().unwrap_or_default(),
                payload.port.unwrap_or(990),
                payload.username.clone(),
                payload.secret.clone(),
                true,
                base_path.clone(),
            ));
            state.register_provider(id.clone(), ftps_fs).await;
        }
        ProviderKind::Sftp => {
            let sftp_fs = Arc::new(SftpFileSystem::new(
                &id,
                payload.host.clone().unwrap_or_default(),
                payload.port.unwrap_or(22),
                payload.username.clone().unwrap_or_else(|| "root".to_string()),
                payload.secret.clone(),
                None,
                base_path.clone(),
            ));
            state.register_provider(id.clone(), sftp_fs).await;
        }
        _ => {}
    }

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "success": true,
            "id": id,
            "message": format!("Connection '{}' created successfully", payload.name),
        })),
    ))
}

/// Delete a connection
pub async fn delete_connection(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    if id == "local" {
        return Err(AppError::BadRequest("Default local connection cannot be deleted".into()));
    }

    sqlx::query("DELETE FROM connections WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to delete connection: {}", e))?;

    state.remove_provider(&id).await;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("Connection '{}' deleted", id)
    })))
}

/// Test connection connectivity
pub async fn test_connection(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    if id == "local" {
        return Ok(Json(TestConnectionResponse {
            success: true,
            latency_ms: 0,
            message: "Local filesystem connected".to_string(),
        }));
    }

    let provider = state
        .get_provider(&id)
        .await
        .ok_or_else(|| VfsError::NotFound(format!("Connection '{}' not found", id)))?;

    // Perform stat test on connection root
    let root_path = crate::domain::VfsPath::root(&id);
    let start = std::time::Instant::now();
    provider.stat(&root_path).await?;
    let latency = start.elapsed().as_millis() as u64;

    Ok(Json(TestConnectionResponse {
        success: true,
        latency_ms: latency,
        message: format!("Connection '{}' verified ({} ms)", id, latency),
    }))
}

/// Get a specific connection and its capabilities
pub async fn get_connection(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let provider = state
        .get_provider(&id)
        .await
        .ok_or_else(|| VfsError::ConnectionError(format!("Connection '{}' not found", id)))?;

    let row: Option<(
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
    )> = sqlx::query_as(
        "SELECT id, name, provider, host, port, username, base_path, read_only, enabled, created_at, updated_at 
         FROM connections WHERE id = ?",
    )
    .bind(&id)
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
        _ => ProviderKind::Local,
    };

    let created_at = DateTime::parse_from_rfc3339(&created_at_str)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
    let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());

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
        status: ConnectionStatus::Connected,
        created_at,
        updated_at,
    };

    Ok(Json(ConnectionDetailResponse {
        connection,
        capabilities: provider.capabilities(),
    }))
}
