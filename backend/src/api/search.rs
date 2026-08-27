use crate::auth::AuthenticatedUser;
use crate::errors::{AppError, VfsError};
use crate::filesystem::search::search_recursive;
use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use utoipa::IntoParams;

#[derive(Debug, Deserialize, IntoParams)]
pub struct SearchQuery {
    pub path: Option<String>,
    pub query: String,
    pub regex: Option<bool>,
    pub max_depth: Option<usize>,
}

/// Recursive search files in a connection
pub async fn search_files(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(connection_id): Path<String>,
    Query(params): Query<SearchQuery>,
) -> Result<impl IntoResponse, AppError> {
    let provider = state
        .get_provider(&connection_id)
        .await
        .ok_or_else(|| VfsError::ConnectionError(format!("Connection '{}' not found", connection_id)))?;

    let start_path = params.path.unwrap_or_else(|| "/".to_string());
    let is_regex = params.regex.unwrap_or(false);
    let max_depth = params.max_depth.unwrap_or(10);

    let results = search_recursive(
        &provider,
        &connection_id,
        &start_path,
        &params.query,
        is_regex,
        max_depth,
    )
    .await?;

    Ok(Json(results))
}
