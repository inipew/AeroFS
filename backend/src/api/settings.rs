use crate::auth::AuthenticatedUser;
use crate::domain::settings::*;
use crate::errors::AppError;
use crate::state::AppState;
use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct SettingsResponse {
    pub settings: AppSettings,
    pub database_url: String,
    pub max_upload_mb: u64,
    // Flat backward-compatible fields
    pub local_root: String,
    pub temp_dir: String,
    pub allow_symlinks: bool,
    pub show_hidden_default: bool,
    pub read_only_default: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateSettingsRequest {
    pub settings: Option<AppSettings>,
    // Flat backward-compatible fields
    pub local_root: Option<String>,
    pub temp_dir: Option<String>,
    pub allow_symlinks: Option<bool>,
    pub show_hidden_default: Option<bool>,
    pub read_only_default: Option<bool>,
}

/// Get current typed system settings and filesystem paths
pub async fn get_settings(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    let local_root = if let Some(custom) = state.get_system_setting("local_root").await {
        custom
    } else {
        state.config.filesystem.default_local_root.to_string_lossy().to_string()
    };

    let temp_dir = if let Some(custom) = state.get_system_setting("temp_dir").await {
        custom
    } else {
        state
            .config
            .filesystem
            .temp_dir
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "./storage/temp".to_string())
    };

    let allow_symlinks = if let Some(val) = state.get_system_setting("allow_symlinks").await {
        val == "true"
    } else {
        state.config.security.allow_symlinks_outside_root
    };

    let show_hidden_default = if let Some(val) = state.get_system_setting("show_hidden_default").await {
        val == "true"
    } else {
        state.config.filesystem.show_hidden_default
    };

    let read_only_default = if let Some(val) = state.get_system_setting("read_only_default").await {
        val == "true"
    } else {
        state.config.filesystem.read_only_default
    };

    let theme = state.get_system_setting("theme").await.unwrap_or_else(|| "dark".to_string());
    let default_view = state.get_system_setting("default_view").await.unwrap_or_else(|| "grid".to_string());
    let default_layout = state.get_system_setting("default_layout").await.unwrap_or_else(|| "split".to_string());
    let max_transfers = state.get_system_setting("max_concurrent_transfers").await
        .and_then(|s| s.parse::<usize>().ok()).unwrap_or(4);
    let retry_attempts = state.get_system_setting("retry_attempts").await
        .and_then(|s| s.parse::<usize>().ok()).unwrap_or(3);
    let conn_timeout = state.get_system_setting("connection_timeout_secs").await
        .and_then(|s| s.parse::<u64>().ok()).unwrap_or(60);
    let log_level = state.get_system_setting("log_level").await.unwrap_or_else(|| "info".to_string());

    let settings = AppSettings {
        general: GeneralSettings {
            language: "en".to_string(),
            theme,
            default_view,
            default_sort: "name".to_string(),
            sort_direction: "asc".to_string(),
            show_hidden_default,
            confirm_destructive: true,
        },
        file_manager: FileManagerSettings {
            default_layout,
            show_breadcrumbs: true,
            show_file_size: true,
            show_permissions: true,
            remember_last_directories: true,
        },
        transfers: TransferSettings {
            max_concurrent_transfers: max_transfers,
            retry_attempts,
            auto_retry: true,
            show_notifications: true,
        },
        connections: ConnectionSettings {
            connection_timeout_secs: conn_timeout,
            health_check_interval_secs: 30,
            auto_reconnect: true,
            default_local_root: local_root.clone(),
            temp_dir: temp_dir.clone(),
        },
        security: SecuritySettings {
            allow_symlinks_outside_root: allow_symlinks,
            confirm_permanent_delete: true,
            read_only_default,
            session_timeout_secs: 86400,
        },
        advanced: AdvancedSettings {
            log_level,
            enable_telemetry: true,
            enable_tracing: true,
            directory_cache_ttl_secs: 0,
        },
    };

    let sanitized_db_url = if user.is_admin {
        state.config.database.url.clone()
    } else {
        "sqlite://[protected]".to_string()
    };

    Ok(Json(SettingsResponse {
        settings,
        database_url: sanitized_db_url,
        max_upload_mb: state.config.limits.max_upload_size / (1024 * 1024),
        local_root,
        temp_dir,
        allow_symlinks,
        show_hidden_default,
        read_only_default,
    }))
}

/// Update typed system settings and apply new configuration dynamically
pub async fn update_settings(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<UpdateSettingsRequest>,
) -> Result<impl IntoResponse, AppError> {
    if !user.is_admin {
        return Err(AppError::Forbidden("Only administrators can update system settings".into()));
    }

    let mut updated_keys = Vec::new();

    if let Some(s) = payload.settings {
        // General
        state.set_system_setting("theme", &s.general.theme).await
            .map_err(|e| anyhow::anyhow!("Failed to save theme: {}", e))?;
        state.set_system_setting("default_view", &s.general.default_view).await
            .map_err(|e| anyhow::anyhow!("Failed to save default_view: {}", e))?;
        state.set_system_setting("show_hidden_default", if s.general.show_hidden_default { "true" } else { "false" }).await
            .map_err(|e| anyhow::anyhow!("Failed to save show_hidden_default: {}", e))?;

        // File Manager
        state.set_system_setting("default_layout", &s.file_manager.default_layout).await
            .map_err(|e| anyhow::anyhow!("Failed to save default_layout: {}", e))?;

        // Transfers
        state.set_system_setting("max_concurrent_transfers", &s.transfers.max_concurrent_transfers.to_string()).await
            .map_err(|e| anyhow::anyhow!("Failed to save max_concurrent_transfers: {}", e))?;
        state.set_system_setting("retry_attempts", &s.transfers.retry_attempts.to_string()).await
            .map_err(|e| anyhow::anyhow!("Failed to save retry_attempts: {}", e))?;

        // Connections & Local Root
        state.set_system_setting("connection_timeout_secs", &s.connections.connection_timeout_secs.to_string()).await
            .map_err(|e| anyhow::anyhow!("Failed to save connection_timeout_secs: {}", e))?;

        let trimmed_root = s.connections.default_local_root.trim();
        if !trimmed_root.is_empty() {
            let path = PathBuf::from(trimmed_root);
            state.update_local_root(path, s.security.allow_symlinks_outside_root).await
                .map_err(|e| anyhow::anyhow!("Failed to update local root: {}", e))?;
            updated_keys.push("local_root");
        }

        let trimmed_temp = s.connections.temp_dir.trim();
        if !trimmed_temp.is_empty() {
            tokio::fs::create_dir_all(trimmed_temp).await
                .map_err(|e| anyhow::anyhow!("Failed to create temp directory: {}", e))?;
            state.set_system_setting("temp_dir", trimmed_temp).await
                .map_err(|e| anyhow::anyhow!("Failed to save temp_dir: {}", e))?;
            updated_keys.push("temp_dir");
        }

        // Security
        state.set_system_setting("allow_symlinks", if s.security.allow_symlinks_outside_root { "true" } else { "false" }).await
            .map_err(|e| anyhow::anyhow!("Failed to save allow_symlinks: {}", e))?;
        state.set_system_setting("read_only_default", if s.security.read_only_default { "true" } else { "false" }).await
            .map_err(|e| anyhow::anyhow!("Failed to save read_only_default: {}", e))?;

        // Advanced
        state.set_system_setting("log_level", &s.advanced.log_level).await
            .map_err(|e| anyhow::anyhow!("Failed to save log_level: {}", e))?;

        updated_keys.push("app_settings_bundle");
    } else {
        // Flat legacy fields fallback
        let allow_symlinks = payload.allow_symlinks.unwrap_or(state.config.security.allow_symlinks_outside_root);

        if let Some(new_root_str) = payload.local_root {
            let trimmed = new_root_str.trim();
            if !trimmed.is_empty() {
                let path = PathBuf::from(trimmed);
                state.update_local_root(path, allow_symlinks).await
                    .map_err(|e| anyhow::anyhow!("Failed to update local root: {}", e))?;
                updated_keys.push("local_root");
            }
        } else if let Some(symlinks) = payload.allow_symlinks {
            state.set_system_setting("allow_symlinks", if symlinks { "true" } else { "false" }).await
                .map_err(|e| anyhow::anyhow!("Failed to update symlinks setting: {}", e))?;
            updated_keys.push("allow_symlinks");
        }

        if let Some(temp) = payload.temp_dir {
            let trimmed = temp.trim();
            if !trimmed.is_empty() {
                tokio::fs::create_dir_all(trimmed).await
                    .map_err(|e| anyhow::anyhow!("Failed to create temp directory: {}", e))?;
                state.set_system_setting("temp_dir", trimmed).await
                    .map_err(|e| anyhow::anyhow!("Failed to update temp directory: {}", e))?;
                updated_keys.push("temp_dir");
            }
        }

        if let Some(show_hidden) = payload.show_hidden_default {
            state.set_system_setting("show_hidden_default", if show_hidden { "true" } else { "false" }).await
                .map_err(|e| anyhow::anyhow!("Failed to update show_hidden: {}", e))?;
            updated_keys.push("show_hidden_default");
        }

        if let Some(ro) = payload.read_only_default {
            state.set_system_setting("read_only_default", if ro { "true" } else { "false" }).await
                .map_err(|e| anyhow::anyhow!("Failed to update read_only: {}", e))?;
            updated_keys.push("read_only_default");
        }
    }

    // Record audit log for system settings change
    let details = format!("Updated keys: {}", updated_keys.join(", "));
    crate::auth::audit::record_audit_log(
        &state.db,
        Some(&user.id),
        "SETTINGS_UPDATED",
        None,
        None,
        "success",
        None,
        Some(&details),
    ).await;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "System and storage path settings updated successfully"
    })))
}
