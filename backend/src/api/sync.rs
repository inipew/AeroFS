use crate::auth::AuthenticatedUser;
use crate::errors::AppError;
use crate::state::AppState;
use crate::sync::models::SyncStrategy;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
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

/// Create a new sync job
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
        Json(serde_json::json!({
            "success": true,
            "job": job,
            "message": "Sync job created successfully and scanning started",
        })),
    ))
}

/// List all sync jobs
pub async fn list_sync_jobs(
    State(state): State<AppState>,
    _user: AuthenticatedUser, // we filter on the client or in manager later, simplified for now
) -> Result<impl IntoResponse, AppError> {
    let jobs = state
        .sync_manager
        .list_jobs()
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(jobs))
}

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

    Ok(Json(serde_json::json!({"success": true})))
}
