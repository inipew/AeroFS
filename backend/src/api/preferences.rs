use crate::auth::AuthenticatedUser;
use crate::domain::settings::UserPreferences;
use crate::errors::AppError;
use crate::state::AppState;
use axum::{extract::State, response::IntoResponse, Json};

/// Get user preferences for currently authenticated user
pub async fn get_user_preferences(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    let prefs: UserPreferences = if let Some(json_str) = state.get_user_preferences(&user.id).await
    {
        serde_json::from_str(&json_str).unwrap_or_default()
    } else {
        UserPreferences::default()
    };

    Ok(Json(prefs))
}

/// Update user preferences for currently authenticated user
pub async fn update_user_preferences(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<UserPreferences>,
) -> Result<impl IntoResponse, AppError> {
    let json_str = serde_json::to_string(&payload)
        .map_err(|e| anyhow::anyhow!("Failed to serialize preferences: {}", e))?;

    state
        .set_user_preferences(&user.id, &json_str)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to persist user preferences: {}", e))?;

    Ok(Json(serde_json::json!({
        "success": true,
        "preferences": payload
    })))
}
