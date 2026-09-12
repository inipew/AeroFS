use crate::domain::{Actor, ConnectionId};
use crate::errors::AppError;
use crate::filesystem::search::{search_recursive, SearchOutput};
use crate::ports::{
    authorization::{Authorization, FileAction},
    filesystem::FileSystemResolver,
};
use std::sync::Arc;
use tokio::sync::Semaphore;

#[derive(Clone)]
pub struct SearchService {
    authorization: Arc<dyn Authorization>,
    filesystem: Arc<dyn FileSystemResolver>,
    limiter: Arc<Semaphore>,
}

impl SearchService {
    pub fn new(
        authorization: Arc<dyn Authorization>,
        filesystem: Arc<dyn FileSystemResolver>,
        limiter: Arc<Semaphore>,
    ) -> Self {
        Self {
            authorization,
            filesystem,
            limiter,
        }
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

        let _permit = self
            .limiter
            .acquire()
            .await
            .map_err(|_| AppError::ServiceUnavailable("Search service is shutting down".into()))?;

        let provider = self.filesystem.resolve(connection).await?;
        let start_path = path_opt.unwrap_or("/");
        let max_depth = max_depth.unwrap_or(10);
        let limit = limit.unwrap_or(500).min(2000);

        search_recursive(
            &provider,
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
