use super::FileApplicationService;
use crate::errors::AppError;

impl FileApplicationService {
    pub async fn write_typed(
        &self,
        _user: &crate::auth::UserInfo,
        _connection: &crate::domain::ConnectionId,
        _path: String,
        _content: Vec<u8>,
    ) -> Result<(), AppError> {
        Err(AppError::Internal(anyhow::anyhow!(
            "write_typed stub — Phase 3 incremental"
        )))
    }
}
