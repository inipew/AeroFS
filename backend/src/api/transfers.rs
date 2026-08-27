use crate::auth::permissions::{check_permission, PermissionAction};
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

/// Queue a new transfer job with full source and destination authorization
pub async fn create_transfer(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<CreateTransferRequest>,
) -> Result<impl IntoResponse, AppError> {
    // 1. Authorize source connection: Read / Download
    check_permission(
        &state.db,
        &user,
        &payload.source_connection_id,
        PermissionAction::Read,
    )
    .await?;

    // If Move transfer, user must also have Delete permission on source connection
    if payload.transfer_type == TransferType::Move {
        check_permission(
            &state.db,
            &user,
            &payload.source_connection_id,
            PermissionAction::Delete,
        )
        .await?;
    }

    // 2. Authorize destination connection: Write / Create
    check_permission(
        &state.db,
        &user,
        &payload.destination_connection_id,
        PermissionAction::Write,
    )
    .await?;
    check_permission(
        &state.db,
        &user,
        &payload.destination_connection_id,
        PermissionAction::Create,
    )
    .await?;

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

/// List all active and historical transfer jobs (scoped by user connection permissions)
pub async fn list_transfers(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    let mut jobs = state.transfer_manager.list_jobs().await;

    if !user.is_admin {
        // Query authorized connections for this user
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT connection_id FROM permissions WHERE user_id = ? AND can_read = 1",
        )
        .bind(&user.id)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

        let mut allowed: std::collections::HashSet<String> =
            rows.into_iter().map(|r| r.0).collect();
        allowed.insert("local".to_string());

        jobs.retain(|j| {
            allowed.contains(&j.source_connection_id)
                && allowed.contains(&j.destination_connection_id)
        });
    }

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
        Err(AppError::NotFound(format!(
            "Transfer job '{}' not running or not found",
            id
        )))
    }
}
