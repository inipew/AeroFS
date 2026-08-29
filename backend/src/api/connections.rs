use crate::auth::AuthenticatedUser;
use crate::errors::AppError;
use crate::services::connection_service::{
    ConnectionService, CreateConnectionRequest, UpdateConnectionRequest,
};
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};

/// List all available connections from database (scoped to user's permissions)
pub async fn list_connections(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    let connections = ConnectionService::list_connections(&state, &user).await?;
    Ok(Json(connections))
}

/// Create a new connection with encrypted credential storage (Admin only, Fail-Closed Transactional)
pub async fn create_connection(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<CreateConnectionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let name = payload.name.clone();
    let id = ConnectionService::create_connection(&state, &user, payload).await?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "success": true,
            "id": id,
            "message": format!("Connection '{}' created successfully", name),
        })),
    ))
}

/// Update an existing connection (Admin only, with atomic hot-swap)
pub async fn update_connection(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<String>,
    Json(payload): Json<UpdateConnectionRequest>,
) -> Result<impl IntoResponse, AppError> {
    ConnectionService::update_connection(&state, &user, &id, payload).await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("Connection '{}' updated successfully", id),
    })))
}

/// Delete a connection
pub async fn delete_connection(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    ConnectionService::delete_connection(&state, &user, &id).await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("Connection '{}' deleted", id),
    })))
}

/// Test connection connectivity
pub async fn test_connection(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let res = ConnectionService::test_connection(&state, &user, &id).await?;
    Ok(Json(res))
}

/// Get a specific connection and its capabilities
pub async fn get_connection(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let res = ConnectionService::get_connection(&state, &user, &id).await?;
    Ok(Json(res))
}
