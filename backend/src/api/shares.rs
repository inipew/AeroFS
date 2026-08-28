use crate::auth::AuthenticatedUser;
use crate::domain::VfsPath;
use crate::errors::AppError;
use crate::services::share_service::{CreateShareRequest, ShareService};
use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use tokio::io::AsyncReadExt;

#[derive(Debug, Deserialize)]
pub struct PublicShareQuery {
    pub password: Option<String>,
}

/// List shares with strict user ownership filter (Admins can view all)
pub async fn list_shares(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    let shares = ShareService::list_shares(&state, &user).await?;
    Ok(Json(shares))
}

/// Create a new shared link for a file or directory
pub async fn create_share(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<CreateShareRequest>,
) -> Result<impl IntoResponse, AppError> {
    let share = ShareService::create_share(&state, &user, payload).await?;
    Ok((StatusCode::CREATED, Json(share)))
}

/// Delete / revoke a shared link
pub async fn delete_share(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(share_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    ShareService::delete_share(&state, &user, &share_id).await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Share link revoked",
    })))
}

/// Public access endpoint for downloading shared files without authentication
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
        mime_type
            .parse()
            .unwrap_or_else(|_| "application/octet-stream".parse().unwrap()),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        format!("inline; filename=\"{}\"", metadata.name)
            .parse()
            .unwrap_or_else(|_| "inline".parse().unwrap()),
    );

    Ok((headers, data))
}
