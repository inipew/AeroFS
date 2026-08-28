use crate::db::DbPool;
use crate::domain::settings::UserPreferences;
use crate::errors::AppError;
use chrono::Utc;

pub struct PreferencesService;

impl PreferencesService {
    pub async fn get_user_preferences(
        db: &DbPool,
        user_id: &str,
    ) -> Result<UserPreferences, AppError> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT preferences_json FROM user_preferences WHERE user_id = ?")
                .bind(user_id)
                .fetch_optional(db)
                .await
                .map_err(|e| anyhow::anyhow!("DB error: {}", e))?;

        let prefs = if let Some(json_str) = row {
            serde_json::from_str(&json_str.0).unwrap_or_default()
        } else {
            UserPreferences::default()
        };

        Ok(prefs)
    }

    pub async fn set_user_preferences(
        db: &DbPool,
        user_id: &str,
        prefs: &UserPreferences,
    ) -> Result<(), AppError> {
        let json_str = serde_json::to_string(prefs)
            .map_err(|e| anyhow::anyhow!("Failed to serialize preferences: {}", e))?;
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO user_preferences (user_id, preferences_json, updated_at) VALUES (?, ?, ?)
             ON CONFLICT(user_id) DO UPDATE SET preferences_json = excluded.preferences_json, updated_at = excluded.updated_at",
        )
        .bind(user_id)
        .bind(json_str)
        .bind(&now)
        .execute(db)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to persist user preferences: {}", e))?;

        Ok(())
    }
}
