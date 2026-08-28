use crate::auth::AuthenticatedUser;
use crate::errors::AppError;
use crate::services::settings_service::{SettingsService, UpdateSettingsRequest};
use crate::state::AppState;
use axum::{extract::State, response::IntoResponse, Json};

/// Get current typed system settings and filesystem paths
pub async fn get_settings(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    let settings = SettingsService::get_settings(&state, &user).await?;
    Ok(Json(settings))
}

/// Update system settings and filesystem paths (Admin only, Atomic Transactional)
pub async fn update_settings(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<UpdateSettingsRequest>,
) -> Result<impl IntoResponse, AppError> {
    SettingsService::update_settings(&state, &user, payload).await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Settings updated successfully"
    })))
}
