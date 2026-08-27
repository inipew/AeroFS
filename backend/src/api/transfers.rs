use crate::auth::AuthenticatedUser;
use crate::errors::AppError;
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

/// Queue a new transfer job
pub async fn create_transfer(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Json(payload): Json<CreateTransferRequest>,
) -> Result<impl IntoResponse, AppError> {
    let job_id = state
        .transfer_manager
        .submit_job(
            payload.name,
            payload.transfer_type,
            payload.source_connection_id,
            payload.source_path,
            payload.destination_connection_id,
            payload.destination_path,
        )
        .await
        .map_err(AppError::BadRequest)?;

    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "success": true,
            "job_id": job_id,
            "message": "Transfer job queued successfully",
        })),
    ))
}

/// List all active and historical transfer jobs
pub async fn list_transfers(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    let jobs = state.transfer_manager.list_jobs().await;
    Ok(Json(jobs))
}

/// Cancel a transfer job
pub async fn cancel_transfer(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let cancelled = state.transfer_manager.cancel_job(&id).await;
    if cancelled {
        Ok(Json(serde_json::json!({
            "success": true,
            "message": format!("Transfer job '{}' cancelled", id),
        })))
    } else {
        Err(AppError::NotFound(format!("Transfer job '{}' not running or not found", id)))
    }
}
