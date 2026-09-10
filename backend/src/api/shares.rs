use crate::api::extractors::{Json, Path, Query};
use crate::auth::AuthenticatedUser;
use crate::domain::VfsPath;
use crate::errors::{AppError, ErrorResponse};
use crate::services::share_service::{CreateShareRequest, ShareItem, ShareService};
use crate::state::AppState;
use axum::{
    extract::State,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;
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
    security(
        ("CookieAuth" = []),
        ("BearerAuth" = [])
    ),
    tag = "shares"
)]
pub async fn list_shares(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    let shares = ShareService::list_shares(&state, &user).await?;
    Ok(Json(shares))
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
    security(
        ("CookieAuth" = []),
        ("BearerAuth" = [])
    ),
    tag = "shares"
)]
pub async fn create_share(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<CreateShareRequest>,
) -> Result<impl IntoResponse, AppError> {
    let share = ShareService::create_share(&state, &user, payload).await?;
    Ok((StatusCode::CREATED, Json(share)))
}

/// Delete / revoke a shared link
#[utoipa::path(
    delete,
    path = "/api/v1/shares/{id}",
    params(
        ("id" = String, Path, description = "Share ID"),
    ),
    responses(
        (status = 200, description = "Share revoked", body = ShareActionResponse),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(
        ("CookieAuth" = []),
        ("BearerAuth" = [])
    ),
    tag = "shares"
)]
pub async fn delete_share(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(share_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    ShareService::delete_share(&state, &user, &share_id).await?;

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
    State(state): State<AppState>,
    Path(token): Path<String>,
    Query(query): Query<PublicShareQuery>,
) -> Result<impl IntoResponse, AppError> {
    let (connection_id, path) =
        ShareService::verify_and_get_public_share(&state, &token, query.password.as_deref())
            .await?;

    let provider = state
        .registry
        .get(&connection_id)
        .await
        .ok_or_else(|| AppError::NotFound("Storage connection not available".into()))?;

    let vfs_path = VfsPath::new(&connection_id, &path)?;
    let metadata = provider.stat(&vfs_path).await?;
    let mut stream = provider.read_stream(&vfs_path).await?;
    let mut data = Vec::new();
    stream
        .read_to_end(&mut data)
        .await
        .map_err(|e| anyhow::anyhow!("Read error: {}", e))?;

    let mime_type = mime_guess::from_path(&metadata.name)
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
        HeaderValue::from_str(&format!("inline; filename=\"{}\"", metadata.name))
            .unwrap_or_else(|_| HeaderValue::from_static("inline")),
    );

    Ok((headers, data))
}
