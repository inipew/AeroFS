use crate::api::extractors::Json;
use crate::auth::AuthenticatedUser;
use crate::domain::settings::UserPreferences;
use crate::errors::{AppError, ErrorResponse};
use crate::state::PreferencesState;
use axum::{extract::State, response::IntoResponse};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct UpdatePreferencesResponse {
    pub success: bool,
    pub preferences: UserPreferences,
}

/// Get user preferences for currently authenticated user
#[utoipa::path(
    get,
    path = "/api/v1/user/preferences",
    responses(
        (status = 200, description = "User preferences", body = UserPreferences),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(("CookieAuth" = []), ("BearerAuth" = [])),
    tag = "preferences"
)]
pub async fn get_user_preferences(
    State(state): State<PreferencesState>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    Ok(Json(state.service.get_user_preferences(&user.id).await?))
}

/// Update user preferences for currently authenticated user
#[utoipa::path(
    put,
    path = "/api/v1/user/preferences",
    request_body = UserPreferences,
    responses(
        (status = 200, description = "Updated user preferences", body = UpdatePreferencesResponse),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(("CookieAuth" = []), ("BearerAuth" = [])),
    tag = "preferences"
)]
pub async fn update_user_preferences(
    State(state): State<PreferencesState>,
    user: AuthenticatedUser,
    Json(payload): Json<UserPreferences>,
) -> Result<impl IntoResponse, AppError> {
    state.service.set_user_preferences(&user.id, &payload).await?;
    Ok(Json(UpdatePreferencesResponse {
        success: true,
        preferences: payload,
    }))
}
