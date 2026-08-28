use crate::auth::AuthenticatedUser;
use crate::domain::settings::UserPreferences;
use crate::errors::AppError;
use crate::services::preferences_service::PreferencesService;
use crate::state::AppState;
use axum::{extract::State, response::IntoResponse, Json};

/// Get user preferences for currently authenticated user
pub async fn get_user_preferences(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    let prefs = PreferencesService::get_user_preferences(&state.db, &user.id).await?;
    Ok(Json(prefs))
}

/// Update user preferences for currently authenticated user
pub async fn update_user_preferences(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<UserPreferences>,
) -> Result<impl IntoResponse, AppError> {
    PreferencesService::set_user_preferences(&state.db, &user.id, &payload).await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "preferences": payload
    })))
}
