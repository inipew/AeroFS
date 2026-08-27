use crate::auth::credentials::{derive_master_key, encrypt_secret};
use crate::auth::permissions::{check_permission, PermissionAction};
use crate::auth::AuthenticatedUser;
use crate::domain::{Capabilities, Connection, ConnectionStatus, ProviderKind};
use crate::errors::{AppError, VfsError};
use crate::state::AppState;
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

/// List all available connections from database (scoped to user's permissions)
pub async fn list_connections(
    State(state): State<AppState>,
    user: AuthenticatedUser,
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
    )> = if user.is_admin {
        sqlx::query_as(
            "SELECT id, name, provider, host, port, username, base_path, read_only, enabled, created_at, updated_at 
             FROM connections WHERE enabled = 1",
        )
        .fetch_all(&state.db)
        .await
        .map_err(|e| anyhow::anyhow!("Database error: {}", e))?
    } else {
        sqlx::query_as(
            "SELECT c.id, c.name, c.provider, c.host, c.port, c.username, c.base_path, c.read_only, c.enabled, c.created_at, c.updated_at 
             FROM connections c
             WHERE c.enabled = 1 AND (c.id = 'local' OR c.id IN (SELECT connection_id FROM permissions WHERE user_id = ? AND can_read = 1))",
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

/// Create a new connection with encrypted credential storage (Admin only, Transactional)
pub async fn create_connection(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<CreateConnectionRequest>,
) -> Result<impl IntoResponse, AppError> {
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

    // Transactional creation: connection + credential
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
            .execute(&mut *tx)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to save credential: {}", e))?;
        }
    }

    tx.commit()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to commit connection transaction: {}", e))?;

    // Register live OpenDAL provider in state
    match payload.provider {
        ProviderKind::Ftp => {
            let host = payload.host.clone().unwrap_or_else(|| "127.0.0.1".into());
            let port = payload.port.unwrap_or(21);
            if let Ok(op) = crate::vfs::opendal::build_ftp_operator(
                &host,
                port,
                false,
                payload.username.as_deref(),
                payload.secret.as_deref(),
                Some(&base_path),
            ) {
                let fs = Arc::new(crate::vfs::opendal::OpenDalFileSystem::new(id.clone(), op));
                state.register_provider(id.clone(), fs).await;
            }
        }
        ProviderKind::Ftps => {
            let host = payload.host.clone().unwrap_or_else(|| "127.0.0.1".into());
            let port = payload.port.unwrap_or(990);
            if let Ok(op) = crate::vfs::opendal::build_ftp_operator(
                &host,
                port,
                true,
                payload.username.as_deref(),
                payload.secret.as_deref(),
                Some(&base_path),
            ) {
                let fs = Arc::new(crate::vfs::opendal::OpenDalFileSystem::new(id.clone(), op));
                state.register_provider(id.clone(), fs).await;
            }
        }
        ProviderKind::Sftp => {
            let host = payload.host.clone().unwrap_or_else(|| "127.0.0.1".into());
            let port = payload.port.unwrap_or(22);
            if let Ok(op) = crate::vfs::opendal::build_sftp_operator(
                &host,
                port,
                payload.username.as_deref(),
                payload.secret.as_deref(),
                Some(&base_path),
            ) {
                let fs = Arc::new(crate::vfs::opendal::OpenDalFileSystem::new(id.clone(), op));
                state.register_provider(id.clone(), fs).await;
            }
        }
        ProviderKind::S3 => {
            let bucket = payload.host.clone().unwrap_or_else(|| "default-bucket".into());
            if let Ok(op) = crate::vfs::opendal::build_s3_operator(
                &bucket,
                None,
                None,
                payload.username.as_deref(),
                payload.secret.as_deref(),
                Some(&base_path),
            ) {
                let fs = Arc::new(crate::vfs::opendal::OpenDalFileSystem::new(id.clone(), op));
                state.register_provider(id.clone(), fs).await;
            }
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

/// Delete a connection (Admin only)
pub async fn delete_connection(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    if !user.is_admin {
        return Err(AppError::Forbidden(
            "Only administrators can delete storage connections".into(),
        ));
    }

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
    user: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    if id == "local" {
        return Ok(Json(TestConnectionResponse {
            success: true,
            latency_ms: 0,
            message: "Local filesystem connected".to_string(),
        }));
    }

    check_permission(&state.db, &user, &id, PermissionAction::Read).await?;

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
    user: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    check_permission(&state.db, &user, &id, PermissionAction::Read).await?;

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
        "s3" => ProviderKind::S3,
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
