use crate::auth::AuthenticatedUser;
use crate::domain::FileMetadata;
use crate::errors::AppError;
use crate::services::file_service::FileService;
use crate::state::AppState;

pub struct PreviewService;

impl PreviewService {
    pub async fn get_preview_info(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        path: &str,
    ) -> Result<FileMetadata, AppError> {
        FileService::stat_file(state, user, connection_id, path).await
    }
}
