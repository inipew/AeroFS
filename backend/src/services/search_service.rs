use crate::domain::{Actor, ConnectionId};
use crate::errors::AppError;
use crate::filesystem::search::{search_recursive, SearchOutput};
use crate::ports::{
    authorization::{Authorization, FileAction},
    filesystem::FileSystemResolver,
};
use crate::runtime::ResourceBudget;
use std::sync::Arc;

#[derive(Clone)]
pub struct SearchService {
    authorization: Arc<dyn Authorization>,
    filesystem: Arc<dyn FileSystemResolver>,
    budget: Arc<ResourceBudget>,
}

impl SearchService {
    pub fn new(
        authorization: Arc<dyn Authorization>,
        filesystem: Arc<dyn FileSystemResolver>,
        budget: Arc<ResourceBudget>,
    ) -> Self {
        Self {
            authorization,
            filesystem,
            budget,
        }
    }

    /// Observable global search-stream capacity for diagnostics/tests.
    pub fn available_capacity(&self) -> usize {
        self.budget.available_search()
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn search_files(
        &self,
        actor: &Actor,
        connection: &ConnectionId,
        path_opt: Option<&str>,
        query: &str,
        is_regex: bool,
        max_depth: Option<usize>,
        limit: Option<usize>,
    ) -> Result<SearchOutput, AppError> {
        self.authorization
            .authorize(actor, connection, FileAction::Read)
            .await?;

        let provider = self.filesystem.resolve(connection).await?;
        let start_path = path_opt.unwrap_or("/");
        let max_depth = max_depth.unwrap_or(10);
        let limit = limit.unwrap_or(500).min(2000);

        search_recursive(
            &provider,
            self.budget.clone(),
            connection.as_str(),
            start_path,
            query,
            is_regex,
            max_depth,
            limit,
        )
        .await
        .map_err(Into::into)
    }
}
