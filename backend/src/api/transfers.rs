use crate::auth::AuthenticatedUser;
use crate::errors::AppError;
use crate::services::TransferService;
use crate::state::AppState;
use crate::transfer::TransferType;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTransferRequest {
    pub name: String,
    pub transfer_type: TransferType,
    pub source_connection_id: String,
    pub source_path: String,
    pub destination_connection_id: String,
    pub destination_path: String,
}

/// Queue a new transfer job with full source and destination authorization
pub async fn create_transfer(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<CreateTransferRequest>,
) -> Result<impl IntoResponse, AppError> {
    let job_id = TransferService::create_transfer(
        &state,
        &user,
        payload.name,
        payload.transfer_type,
        payload.source_connection_id,
        payload.source_path,
        payload.destination_connection_id,
        payload.destination_path,
    )
    .await?;

    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "success": true,
            "job_id": job_id,
            "message": "Transfer job queued successfully",
        })),
    ))
}

/// List active and undismissed transfer jobs (scoped by user ownership and connection permissions)
pub async fn list_transfers(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    let jobs = TransferService::list_transfers(&state, &user).await?;
    Ok(Json(jobs))
}

/// Cancel an active transfer job (enforcing user ownership)
pub async fn cancel_transfer(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    TransferService::cancel_transfer(&state, &user, &id).await?;
    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("Transfer job '{}' cancelled", id),
    })))
}

/// Retry or resume an interrupted or failed transfer job
pub async fn retry_transfer(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    TransferService::retry_transfer(&state, &user, &id).await?;
    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("Transfer job '{}' queued for retry", id),
    })))
}

/// Dismiss a single transfer job from history (persistent)
pub async fn dismiss_transfer(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    TransferService::dismiss_transfer(&state, &user, &id).await?;
    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("Transfer job '{}' dismissed", id),
    })))
}

/// Dismiss all finished transfer jobs for the authenticated user (persistent Clear)
pub async fn clear_finished_transfers(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    let cleared = TransferService::clear_finished_transfers(&state, &user).await?;
    Ok(Json(serde_json::json!({
        "success": true,
        "cleared": cleared,
        "message": format!("Cleared {} finished transfer(s)", cleared),
    })))
}
