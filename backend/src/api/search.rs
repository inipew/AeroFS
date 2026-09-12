use crate::api::extractors::{Json, Path, Query};
use crate::auth::AuthenticatedUser;
use crate::domain::{Actor, ConnectionId};
use crate::errors::{AppError, ErrorResponse};
use crate::filesystem::search::SearchOutput;
use crate::state::SearchState;
use axum::{extract::State, response::IntoResponse};
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct SearchQuery {
    pub path: Option<String>,
    pub query: String,
    pub regex: Option<bool>,
    pub max_depth: Option<usize>,
    pub limit: Option<usize>,
}

/// Recursive search files in a connection
#[utoipa::path(
    get,
    path = "/api/v1/connections/{connection_id}/search",
    params(
        ("connection_id" = String, Path, description = "Connection ID"),
        SearchQuery
    ),
    responses(
        (status = 200, description = "Search results", body = SearchOutput),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(
        ("CookieAuth" = []),
        ("BearerAuth" = [])
    ),
    tag = "search"
)]
pub async fn search_files(
    State(state): State<SearchState>,
    user: AuthenticatedUser,
    Path(connection_id): Path<String>,
    Query(params): Query<SearchQuery>,
) -> Result<impl IntoResponse, AppError> {
    let connection = ConnectionId::new(connection_id)
        .map_err(|error| AppError::BadRequest(error.to_string()))?;
    let actor = Actor {
        id: user.id.clone(),
        username: user.username.clone(),
        is_admin: user.is_admin,
    };
    let output = state
        .service
        .search_files(
            &actor,
            &connection,
            params.path.as_deref(),
            &params.query,
            params.regex.unwrap_or(false),
            params.max_depth,
            params.limit,
        )
        .await?;

    Ok(Json(output))
}
