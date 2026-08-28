use crate::auth::permissions::{check_permission, PermissionAction};
use crate::auth::AuthenticatedUser;
use crate::errors::{AppError, VfsError};
use crate::filesystem::search::{search_recursive, SearchOutput};
use crate::state::AppState;

pub struct SearchService;

impl SearchService {
    #[allow(clippy::too_many_arguments)]
    pub async fn search_files(
        state: &AppState,
        user: &AuthenticatedUser,
        connection_id: &str,
        path_opt: Option<&str>,
        query: &str,
        is_regex: bool,
        max_depth: Option<usize>,
        limit: Option<usize>,
    ) -> Result<SearchOutput, AppError> {
        check_permission(&state.db, user, connection_id, PermissionAction::Read).await?;

        let _permit = state.search_semaphore.acquire().await;

        let provider = state.registry.get(connection_id).await.ok_or_else(|| {
            VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
        })?;

        let start_path = path_opt.unwrap_or("/");
        let max_depth = max_depth.unwrap_or(10);
        let limit = limit.unwrap_or(500).min(2000);

        let output = search_recursive(
            &provider,
            connection_id,
            start_path,
            query,
            is_regex,
            max_depth,
            limit,
        )
        .await?;

        Ok(output)
    }
}
