use crate::auth::AuthenticatedUser;
use crate::errors::AppError;
use crate::state::AppState;
use crate::sync::models::{FileManifest, SyncStrategy};
use axum::{
    extract::State,
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
    #[serde(default)]
    pub source_manifest: Option<Vec<FileManifest>>,
    #[serde(default)]
    pub destination_manifest: Option<Vec<FileManifest>>,
}

/// Create a new sync job and optionally trigger manifest reconciliation
pub async fn create_sync_job(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<CreateSyncRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Check user permission on source (Read) and destination (Write, Create)
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

    crate::auth::permissions::check_permission(
        &state.db,
        &user,
        &payload.destination_connection_id,
        crate::auth::permissions::PermissionAction::Create,
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

    let mut reconciled_count = 0;
    if let (Some(src_manifest), Some(dst_manifest)) = (payload.source_manifest, payload.destination_manifest) {
        reconciled_count = state
            .sync_manager
            .execute_reconciliation(&job.id, src_manifest, dst_manifest)
            .await
            .map_err(AppError::Internal)?;
    }

    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "success": true,
            "job": job,
            "transfers_submitted": reconciled_count,
            "message": "Sync job created successfully",
        })),
    ))
}

/// List all sync jobs for the authenticated user
pub async fn list_sync_jobs(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    let jobs = state
        .sync_manager
        .list_jobs(&user.id)
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(jobs))
}
