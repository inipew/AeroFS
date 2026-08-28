use crate::auth::AuthenticatedUser;
use crate::errors::AppError;
use crate::services::search_service::SearchService;
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
    pub limit: Option<usize>,
}

/// Recursive search files in a connection
pub async fn search_files(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(connection_id): Path<String>,
    Query(params): Query<SearchQuery>,
) -> Result<impl IntoResponse, AppError> {
    let output = SearchService::search_files(
        &state,
        &user,
        &connection_id,
        params.path.as_deref(),
        &params.query,
        params.regex.unwrap_or(false),
        params.max_depth,
        params.limit,
    )
    .await?;

    Ok(Json(output))
}
