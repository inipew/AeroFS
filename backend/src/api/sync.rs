use crate::api::extractors::{Json, Path};
use crate::auth::AuthenticatedUser;
use crate::errors::{AppError, ErrorResponse};
use crate::state::AppState;
use crate::sync::models::{SyncJob, SyncOperation, SyncStrategy};
use axum::{extract::State, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateSyncRequest {
    pub source_connection_id: String,
    pub source_path: String,
    pub destination_connection_id: String,
    pub destination_path: String,
    #[serde(default)]
    pub strategy: SyncStrategy,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ResolveConflictRequest {
    pub op_id: String,
    pub resolution: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CreateSyncResponse {
    pub success: bool,
    pub job: SyncJob,
    pub message: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ResolveConflictResponse {
    pub success: bool,
}

/// Create a new sync job
#[utoipa::path(
    post,
    path = "/api/v1/sync",
    request_body = CreateSyncRequest,
    responses(
        (status = 202, description = "Sync job created", body = CreateSyncResponse),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(
        ("CookieAuth" = []),
        ("BearerAuth" = [])
    ),
    tag = "sync"
)]
pub async fn create_sync_job(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<CreateSyncRequest>,
) -> Result<impl IntoResponse, AppError> {
    crate::auth::permissions::check_permission(
        &state.db,
        &user,
        &payload.source_connection_id,
        crate::auth::permissions::PermissionAction::Read,
    )
    .await?;

    crate::auth::permissions::check_permission(
        &state.db,
        &user,
        &payload.destination_connection_id,
        crate::auth::permissions::PermissionAction::Write,
    )
    .await?;

    let job = state
        .sync_manager
        .create_job(
            &user.id,
            &payload.source_connection_id,
            &payload.source_path,
            &payload.destination_connection_id,
            &payload.destination_path,
            payload.strategy,
        )
        .await
        .map_err(AppError::Internal)?;

    Ok((
        StatusCode::ACCEPTED,
        Json(CreateSyncResponse {
            success: true,
            job,
            message: "Sync job created successfully and scanning started".to_string(),
        }),
    ))
}

/// List all sync jobs
#[utoipa::path(
    get,
    path = "/api/v1/sync",
    responses(
        (status = 200, description = "List of sync jobs", body = Vec<SyncJob>),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(
        ("CookieAuth" = []),
        ("BearerAuth" = [])
    ),
    tag = "sync"
)]
pub async fn list_sync_jobs(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    let jobs = state
        .sync_manager
        .list_jobs()
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(jobs))
}

/// List operations for a sync job
#[utoipa::path(
    get,
    path = "/api/v1/sync/{id}/operations",
    params(
        ("id" = String, Path, description = "Sync job ID"),
    ),
    responses(
        (status = 200, description = "List of sync operations", body = Vec<SyncOperation>),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(
        ("CookieAuth" = []),
        ("BearerAuth" = [])
    ),
    tag = "sync"
)]
pub async fn list_operations(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let ops = state
        .sync_manager
        .list_operations(&id)
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(ops))
}

/// Resolve a conflict in a sync job
#[utoipa::path(
    post,
    path = "/api/v1/sync/{id}/resolve",
    params(
        ("id" = String, Path, description = "Sync job ID"),
    ),
    request_body = ResolveConflictRequest,
    responses(
        (status = 200, description = "Conflict resolved", body = ResolveConflictResponse),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(
        ("CookieAuth" = []),
        ("BearerAuth" = [])
    ),
    tag = "sync"
)]
pub async fn resolve_conflict(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(id): Path<String>,
    Json(payload): Json<ResolveConflictRequest>,
) -> Result<impl IntoResponse, AppError> {
    state
        .sync_manager
        .resolve_conflict(&id, &payload.op_id, &payload.resolution)
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(ResolveConflictResponse { success: true }))
}
