use crate::domain::settings::UserPreferences;
use crate::errors::AppError;
use crate::ports::audit::UserPreferencesRepository;
use std::sync::Arc;

#[derive(Clone)]
pub struct PreferencesService {
    repository: Arc<dyn UserPreferencesRepository>,
}

impl PreferencesService {
    pub fn new(repository: Arc<dyn UserPreferencesRepository>) -> Self {
        Self { repository }
    }

    pub async fn get_user_preferences(&self, user_id: &str) -> Result<UserPreferences, AppError> {
        Ok(self.repository.get(user_id).await?.unwrap_or_default())
    }

    pub async fn set_user_preferences(
        &self,
        user_id: &str,
        prefs: &UserPreferences,
    ) -> Result<(), AppError> {
        self.repository.set(user_id, prefs).await
    }
}
