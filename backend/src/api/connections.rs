use crate::api::extractors::{Json, Path};
use crate::auth::AuthenticatedUser;
use crate::domain::Actor;
use crate::errors::{AppError, ErrorResponse};
use crate::services::connection_service::{
    ConnectionDetailResponse, ConnectionService, CreateConnectionRequest, TestConnectionResponse,
    UpdateConnectionRequest,
};
use crate::state::AppState;
use axum::{extract::{FromRef, State}, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone)]
pub struct ConnectionState {
    pub service: ConnectionService,
}

impl FromRef<AppState> for ConnectionState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            service: ConnectionService::new(
                state.db.clone(),
                state.config.clone(),
                state.registry.clone(),
                state.credentials.clone(),
                state.metadata_cache.clone(),
                state.transfer_manager.clone(),
            ),
        }
    }
}

fn actor(user: &AuthenticatedUser) -> Actor {
    Actor {
        id: user.id().to_string(),
        username: user.username().to_string(),
        is_admin: user.is_admin(),
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateConnectionResponse {
    pub success: bool,
    pub id: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ConnectionActionResponse {
    pub success: bool,
    pub message: String,
}

/// List all available connections from database (scoped to user's permissions)
#[utoipa::path(
    get,
    path = "/api/v1/connections",
    responses(
        (status = 200, description = "List of storage connections", body = Vec<ConnectionDetailResponse>),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(("CookieAuth" = []), ("BearerAuth" = [])),
    tag = "connections"
)]
pub async fn list_connections(
    State(state): State<ConnectionState>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    Ok(axum::Json(state.service.list_connections(&actor(&user)).await?))
}

/// Create a new connection with encrypted credential storage (Admin only, Fail-Closed Transactional)
#[utoipa::path(
    post,
    path = "/api/v1/connections",
    request_body = CreateConnectionRequest,
    responses(
        (status = 201, description = "Connection created successfully", body = CreateConnectionResponse),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden - Admin required", body = ErrorResponse),
        (status = 409, description = "Conflict - Connection name already exists", body = ErrorResponse)
    ),
    security(("CookieAuth" = []), ("BearerAuth" = [])),
    tag = "connections"
)]
pub async fn create_connection(
    State(state): State<ConnectionState>,
    user: AuthenticatedUser,
    Json(payload): Json<CreateConnectionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let name = payload.name.clone();
    let id = state.service.create_connection(&actor(&user), payload).await?;
    Ok((
        StatusCode::CREATED,
        axum::Json(CreateConnectionResponse {
            success: true,
            id,
            message: format!("Connection '{}' created successfully", name),
        }),
    ))
}

/// Update an existing connection (Admin only, with atomic hot-swap)
#[utoipa::path(
    put,
    path = "/api/v1/connections/{id}",
    params(("id" = String, Path, description = "Connection identifier")),
    request_body = UpdateConnectionRequest,
    responses(
        (status = 200, description = "Connection updated successfully", body = ConnectionActionResponse),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden - Admin required", body = ErrorResponse),
        (status = 404, description = "Connection not found", body = ErrorResponse)
    ),
    security(("CookieAuth" = []), ("BearerAuth" = [])),
    tag = "connections"
)]
pub async fn update_connection(
    State(state): State<ConnectionState>,
    user: AuthenticatedUser,
    Path(id): Path<String>,
    Json(payload): Json<UpdateConnectionRequest>,
) -> Result<impl IntoResponse, AppError> {
    state.service.update_connection(&actor(&user), &id, payload).await?;
    Ok(axum::Json(ConnectionActionResponse {
        success: true,
        message: format!("Connection '{}' updated successfully", id),
    }))
}

/// Delete a connection
#[utoipa::path(
    delete,
    path = "/api/v1/connections/{id}",
    params(("id" = String, Path, description = "Connection identifier")),
    responses(
        (status = 200, description = "Connection deleted successfully", body = ConnectionActionResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden - Admin required", body = ErrorResponse),
        (status = 404, description = "Connection not found", body = ErrorResponse)
    ),
    security(("CookieAuth" = []), ("BearerAuth" = [])),
    tag = "connections"
)]
pub async fn delete_connection(
    State(state): State<ConnectionState>,
    user: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    state.service.delete_connection(&actor(&user), &id).await?;
    Ok(axum::Json(ConnectionActionResponse {
        success: true,
        message: format!("Connection '{}' deleted", id),
    }))
}

/// Test connection connectivity
#[utoipa::path(
    post,
    path = "/api/v1/connections/{id}/test",
    params(("id" = String, Path, description = "Connection identifier")),
    responses(
        (status = 200, description = "Connection test completed", body = TestConnectionResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Connection not found", body = ErrorResponse)
    ),
    security(("CookieAuth" = []), ("BearerAuth" = [])),
    tag = "connections"
)]
pub async fn test_connection(
    State(state): State<ConnectionState>,
    user: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    Ok(axum::Json(state.service.test_connection(&actor(&user), &id).await?))
}

/// Get a specific connection and its capabilities
#[utoipa::path(
    get,
    path = "/api/v1/connections/{id}",
    params(("id" = String, Path, description = "Connection identifier")),
    responses(
        (status = 200, description = "Connection details and capabilities", body = ConnectionDetailResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Connection not found", body = ErrorResponse)
    ),
    security(("CookieAuth" = []), ("BearerAuth" = [])),
    tag = "connections"
)]
pub async fn get_connection(
    State(state): State<ConnectionState>,
    user: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    Ok(axum::Json(state.service.get_connection(&actor(&user), &id).await?))
}
