use crate::auth::AuthenticatedUser;
use crate::errors::AppError;
use crate::state::AppState;
use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemSettingsResponse {
    pub local_root: String,
    pub temp_dir: String,
    pub database_url: String,
    pub allow_symlinks: bool,
    pub show_hidden_default: bool,
    pub read_only_default: bool,
    pub max_upload_mb: u64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateSettingsRequest {
    pub local_root: Option<String>,
    pub temp_dir: Option<String>,
    pub allow_symlinks: Option<bool>,
    pub show_hidden_default: Option<bool>,
    pub read_only_default: Option<bool>,
}

/// Get current system settings and filesystem paths
pub async fn get_settings(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
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

    Ok(Json(SystemSettingsResponse {
        local_root,
        temp_dir,
        database_url: state.config.database.url.clone(),
        allow_symlinks,
        show_hidden_default,
        read_only_default,
        max_upload_mb: state.config.limits.max_upload_size / (1024 * 1024),
    }))
}

/// Update system settings and apply new root path dynamically
pub async fn update_settings(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<UpdateSettingsRequest>,
) -> Result<impl IntoResponse, AppError> {
    if !user.is_admin {
        return Err(AppError::Forbidden("Only administrators can update system settings".into()));
    }

    let allow_symlinks = payload.allow_symlinks.unwrap_or(state.config.security.allow_symlinks_outside_root);

    if let Some(new_root_str) = payload.local_root {
        let trimmed = new_root_str.trim();
        if !trimmed.is_empty() {
            let path = PathBuf::from(trimmed);
            state.update_local_root(path, allow_symlinks).await
                .map_err(|e| anyhow::anyhow!("Failed to update local root: {}", e))?;
        }
    } else if let Some(symlinks) = payload.allow_symlinks {
        state.set_system_setting("allow_symlinks", if symlinks { "true" } else { "false" }).await
            .map_err(|e| anyhow::anyhow!("Failed to update symlinks setting: {}", e))?;
    }

    if let Some(temp) = payload.temp_dir {
        let trimmed = temp.trim();
        if !trimmed.is_empty() {
            let _ = tokio::fs::create_dir_all(trimmed).await;
            state.set_system_setting("temp_dir", trimmed).await
                .map_err(|e| anyhow::anyhow!("Failed to update temp directory: {}", e))?;
        }
    }

    if let Some(show_hidden) = payload.show_hidden_default {
        state.set_system_setting("show_hidden_default", if show_hidden { "true" } else { "false" }).await
            .map_err(|e| anyhow::anyhow!("Failed to update show_hidden: {}", e))?;
    }

    if let Some(ro) = payload.read_only_default {
        state.set_system_setting("read_only_default", if ro { "true" } else { "false" }).await
            .map_err(|e| anyhow::anyhow!("Failed to update read_only: {}", e))?;
    }

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "System and storage path settings updated successfully"
    })))
}
