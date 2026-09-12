use crate::api::extractors::{Json, Path};
use crate::auth::AuthenticatedUser;
use crate::errors::{AppError, ErrorResponse};
use crate::state::TransferState;
use crate::transfer::model::TransferJobResponse;
use crate::transfer::TransferType;
use axum::{extract::State, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Serialize, ToSchema)]
pub struct CreateTransferResponse {
    pub success: bool,
    pub job_id: String,
    pub message: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TransferActionResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ClearFinishedTransfersResponse {
    pub success: bool,
    pub cleared: usize,
    pub message: String,
}

fn actor(user: &AuthenticatedUser) -> crate::domain::Actor {
    crate::domain::Actor {
        id: user.id.clone(),
        username: user.username.clone(),
        is_admin: user.is_admin,
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/transfers",
    request_body = CreateTransferRequest,
    responses(
        (status = 202, description = "Transfer job queued", body = CreateTransferResponse),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(("CookieAuth" = []), ("BearerAuth" = [])),
    tag = "transfers"
)]
pub async fn create_transfer(
    State(state): State<TransferState>,
    user: AuthenticatedUser,
    Json(payload): Json<CreateTransferRequest>,
) -> Result<impl IntoResponse, AppError> {
    let source_connection = crate::domain::ConnectionId::new(payload.source_connection_id)
        .map_err(|error| AppError::BadRequest(error.to_string()))?;
    let destination_connection = crate::domain::ConnectionId::new(payload.destination_connection_id)
        .map_err(|error| AppError::BadRequest(error.to_string()))?;
    let job_id = state
        .use_cases
        .create_transfer
        .execute(
            &actor(&user),
            crate::application::transfers::CreateTransferCommand {
                name: payload.name,
                transfer_type: payload.transfer_type,
                source_connection,
                source_path: payload.source_path,
                destination_connection,
                destination_path: payload.destination_path,
            },
        )
        .await?;

    Ok((
        StatusCode::ACCEPTED,
        Json(CreateTransferResponse {
            success: true,
            job_id,
            message: "Transfer job queued successfully".to_string(),
        }),
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/transfers",
    responses(
        (status = 200, description = "List of transfer jobs", body = Vec<TransferJobResponse>),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(("CookieAuth" = []), ("BearerAuth" = [])),
    tag = "transfers"
)]
pub async fn list_transfers(
    State(state): State<TransferState>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    Ok(Json(state.use_cases.list(&actor(&user)).await?))
}

#[utoipa::path(
    post,
    path = "/api/v1/transfers/{id}/cancel",
    params(("id" = String, Path, description = "Transfer job ID")),
    responses(
        (status = 200, description = "Transfer cancelled", body = TransferActionResponse),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Not found", body = ErrorResponse),
        (status = 409, description = "Conflict / Not cancellable", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(("CookieAuth" = []), ("BearerAuth" = [])),
    tag = "transfers"
)]
pub async fn cancel_transfer(
    State(state): State<TransferState>,
    user: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    state.use_cases.cancel(&actor(&user), &id).await?;
    Ok(Json(TransferActionResponse {
        success: true,
        message: format!("Transfer job '{}' cancelled", id),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/transfers/{id}/retry",
    params(("id" = String, Path, description = "Transfer job ID")),
    responses(
        (status = 200, description = "Transfer queued for retry", body = TransferActionResponse),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(("CookieAuth" = []), ("BearerAuth" = [])),
    tag = "transfers"
)]
pub async fn retry_transfer(
    State(state): State<TransferState>,
    user: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    state.use_cases.retry(&actor(&user), &id).await?;
    Ok(Json(TransferActionResponse {
        success: true,
        message: format!("Transfer job '{}' queued for retry", id),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/transfers/{id}/dismiss",
    params(("id" = String, Path, description = "Transfer job ID")),
    responses(
        (status = 200, description = "Transfer dismissed", body = TransferActionResponse),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(("CookieAuth" = []), ("BearerAuth" = [])),
    tag = "transfers"
)]
pub async fn dismiss_transfer(
    State(state): State<TransferState>,
    user: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    state.use_cases.dismiss(&actor(&user), &id).await?;
    Ok(Json(TransferActionResponse {
        success: true,
        message: format!("Transfer job '{}' dismissed", id),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/transfers/clear-finished",
    responses(
        (status = 200, description = "Finished transfers cleared", body = ClearFinishedTransfersResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(("CookieAuth" = []), ("BearerAuth" = [])),
    tag = "transfers"
)]
pub async fn clear_finished_transfers(
    State(state): State<TransferState>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    let cleared = state.use_cases.clear_finished(&actor(&user)).await?;
    Ok(Json(ClearFinishedTransfersResponse {
        success: true,
        cleared,
        message: format!("Cleared {} finished transfer(s)", cleared),
    }))
}
