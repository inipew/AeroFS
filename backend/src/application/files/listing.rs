use super::FileApplicationService;
use crate::auth::UserInfo;
use crate::domain::{DirectoryListing, SortField, SortOrder};
use crate::errors::AppError;

#[derive(Debug, Clone)]
pub struct ListOptions {
    pub path: Option<String>,
    pub show_hidden: Option<bool>,
    pub sort: Option<SortField>,
    pub order: Option<SortOrder>,
    pub cursor: Option<String>,
    pub limit: Option<usize>,
}

impl FileApplicationService {
    /// Typed listing — new API boundary (§61, §129).
    /// Wraps legacy stringly service while exposing typed options to handlers.
    /// Incremental: still delegates to FileService via AppState shim; next step removes &AppState.
    pub async fn list_paged_typed(
        &self,
        state: &crate::state::AppState,
        user: &UserInfo,
        connection: &crate::domain::ConnectionId,
        opts: ListOptions,
    ) -> Result<DirectoryListing, AppError> {
        let sort_str = opts.sort.map(|s| match s {
            SortField::Name => "name",
            SortField::Size => "size",
            SortField::Modified => "modified",
        });
        let order_str = opts.order.map(|o| match o {
            SortOrder::Asc => "asc",
            SortOrder::Desc => "desc",
        });
        // Delegate to existing FileService (still &AppState) — typed wrapper shows intended boundary
        crate::services::FileService::list_directory_paged(
            state,
            &crate::auth::AuthenticatedUser(user.clone()),
            connection.as_str(),
            opts.path,
            opts.show_hidden,
            sort_str,
            order_str,
            opts.cursor.as_deref(),
            opts.limit,
        )
        .await
    }
}
