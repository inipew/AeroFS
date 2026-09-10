use crate::api::extractors::Json;
use crate::auth::AuthenticatedUser;
use crate::errors::{AppError, ErrorResponse};
use crate::services::settings_service::{SettingsResponse, SettingsService, UpdateSettingsRequest};
use crate::state::AppState;
use axum::{extract::State, response::IntoResponse};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct UpdateSettingsResponse {
    pub success: bool,
    pub message: String,
}

/// Get current typed system settings and filesystem paths
#[utoipa::path(
    get,
    path = "/api/v1/settings",
    responses(
        (status = 200, description = "System settings", body = SettingsResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(
        ("CookieAuth" = []),
        ("BearerAuth" = [])
    ),
    tag = "settings"
)]
pub async fn get_settings(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    let settings = SettingsService::get_settings(&state, &user).await?;
    Ok(Json(settings))
}

/// Update system settings and filesystem paths (Admin only, Atomic Transactional)
#[utoipa::path(
    put,
    path = "/api/v1/settings",
    request_body = UpdateSettingsRequest,
    responses(
        (status = 200, description = "Settings updated", body = UpdateSettingsResponse),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(
        ("CookieAuth" = []),
        ("BearerAuth" = [])
    ),
    tag = "settings"
)]
pub async fn update_settings(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<UpdateSettingsRequest>,
) -> Result<impl IntoResponse, AppError> {
    SettingsService::update_settings(&state, &user, payload).await?;

    Ok(Json(UpdateSettingsResponse {
        success: true,
        message: "Settings updated successfully".to_string(),
    }))
}
