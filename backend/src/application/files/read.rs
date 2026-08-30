use super::FileApplicationService;
use crate::domain::VfsPath;
use crate::errors::AppError;

#[derive(Debug, Clone)]
pub struct ReadOptions {
    pub path: String,
    pub download: bool,
    pub range: Option<String>,
}

impl FileApplicationService {
    pub async fn read_typed(
        &self,
        _user: &crate::auth::UserInfo,
        _connection: &crate::domain::ConnectionId,
        _opts: ReadOptions,
    ) -> Result<(Vec<u8>, VfsPath), AppError> {
        Err(AppError::Internal(anyhow::anyhow!(
            "read_typed stub — Phase 3 incremental"
        )))
    }
}
