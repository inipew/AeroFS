use crate::api::extractors::{Json, Path, Query};
use crate::auth::AuthenticatedUser;
use crate::errors::{AppError, ErrorResponse};
use crate::services::share_service::{CreateShareRequest, ShareItem};
use crate::state::ShareState;
use axum::{
    extract::State,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct PublicShareQuery {
    pub password: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ShareActionResponse {
    pub success: bool,
    pub message: String,
}

/// List shares with strict user ownership filter (Admins can view all)
#[utoipa::path(
    get,
    path = "/api/v1/shares",
    responses(
        (status = 200, description = "List of shares", body = Vec<ShareItem>),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(("CookieAuth" = []), ("BearerAuth" = [])),
    tag = "shares"
)]
pub async fn list_shares(
    State(state): State<ShareState>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    Ok(Json(state.service.list_shares(&user).await?))
}

/// Create a new shared link for a file or directory
#[utoipa::path(
    post,
    path = "/api/v1/shares",
    request_body = CreateShareRequest,
    responses(
        (status = 201, description = "Share created", body = ShareItem),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(("CookieAuth" = []), ("BearerAuth" = [])),
    tag = "shares"
)]
pub async fn create_share(
    State(state): State<ShareState>,
    user: AuthenticatedUser,
    Json(payload): Json<CreateShareRequest>,
) -> Result<impl IntoResponse, AppError> {
    Ok((StatusCode::CREATED, Json(state.service.create_share(&user, payload).await?)))
}

/// Delete / revoke a shared link
#[utoipa::path(
    delete,
    path = "/api/v1/shares/{id}",
    params(("id" = String, Path, description = "Share ID")),
    responses(
        (status = 200, description = "Share revoked", body = ShareActionResponse),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(("CookieAuth" = []), ("BearerAuth" = [])),
    tag = "shares"
)]
pub async fn delete_share(
    State(state): State<ShareState>,
    user: AuthenticatedUser,
    Path(share_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    state.service.delete_share(&user, &share_id).await?;
    Ok(Json(ShareActionResponse {
        success: true,
        message: "Share link revoked".to_string(),
    }))
}

/// Public access endpoint for downloading shared files without authentication
#[utoipa::path(
    get,
    path = "/api/v1/shares/public/{token}",
    params(
        ("token" = String, Path, description = "Public share token"),
        PublicShareQuery
    ),
    responses(
        (status = 200, description = "File content stream", content_type = "application/octet-stream"),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Password required or invalid", body = ErrorResponse),
        (status = 404, description = "Share expired or not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "shares"
)]
pub async fn public_get_share(
    State(state): State<ShareState>,
    Path(token): Path<String>,
    Query(query): Query<PublicShareQuery>,
) -> Result<impl IntoResponse, AppError> {
    let content = state
        .service
        .read_public_share(&token, query.password.as_deref())
        .await?;
    let mime_type = mime_guess::from_path(&content.name)
        .first_or_octet_stream()
        .to_string();
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&mime_type)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("inline; filename=\"{}\"", content.name))
            .unwrap_or_else(|_| HeaderValue::from_static("inline")),
    );
    Ok((headers, content.data))
}
