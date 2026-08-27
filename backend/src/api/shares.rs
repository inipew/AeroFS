use crate::auth::AuthenticatedUser;
use crate::errors::AppError;
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct ShareItem {
    pub id: String,
    pub connection_id: String,
    pub path: String,
    pub share_token: String,
    pub has_password: bool,
    pub expires_at: Option<String>,
    pub created_at: String,
    pub share_url: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateShareRequest {
    pub connection_id: String,
    pub path: String,
    pub password: Option<String>,
    pub expires_in_hours: Option<i64>, // e.g. 24, 168 (7 days), or None
}

/// List all shares
pub async fn list_shares(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    let rows: Vec<(
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        String,
    )> = sqlx::query_as(
        "SELECT id, connection_id, path, share_token, password_hash, expires_at, created_at FROM shares ORDER BY created_at DESC"
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

    let shares: Vec<ShareItem> = rows
        .into_iter()
        .map(|(id, connection_id, path, share_token, pass_hash, expires_at, created_at)| {
            let share_url = format!("/api/v1/shares/public/{}", share_token);
            ShareItem {
                id,
                connection_id,
                path,
                share_token,
                has_password: pass_hash.is_some(),
                expires_at,
                created_at,
                share_url,
            }
        })
        .collect();

    Ok(Json(shares))
}

/// Create a new share link
pub async fn create_share(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<CreateShareRequest>,
) -> Result<impl IntoResponse, AppError> {
    let id = Uuid::new_v4().to_string();
    let share_token = format!("sh_{}", &Uuid::new_v4().to_string().replace('-', "")[..16]);
    let now = Utc::now();
    let now_str = now.to_rfc3339();

    let expires_at_str = payload.expires_in_hours.map(|h| {
        (now + Duration::hours(h)).to_rfc3339()
    });

    let password_hash = payload.password.and_then(|p| {
        if p.trim().is_empty() {
            None
        } else {
            crate::auth::password::hash_password(&p).ok()
        }
    });

    sqlx::query(
        "INSERT INTO shares (id, connection_id, path, share_token, password_hash, expires_at, created_at, created_by)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&id)
    .bind(&payload.connection_id)
    .bind(&payload.path)
    .bind(&share_token)
    .bind(&password_hash)
    .bind(&expires_at_str)
    .bind(&now_str)
    .bind(&user.username)
    .execute(&state.db)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to create share: {}", e))?;

    Ok((
        StatusCode::CREATED,
        Json(ShareItem {
            id,
            connection_id: payload.connection_id,
            path: payload.path,
            share_token: share_token.clone(),
            has_password: password_hash.is_some(),
            expires_at: expires_at_str,
            created_at: now_str,
            share_url: format!("/api/v1/shares/public/{}", share_token),
        }),
    ))
}

/// Delete a share link
pub async fn delete_share(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    sqlx::query("DELETE FROM shares WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to delete share: {}", e))?;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Share link revoked successfully"
    })))
}

/// Public download / view of shared file
pub async fn public_get_share(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let row: Option<(String, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT connection_id, path, password_hash, expires_at FROM shares WHERE share_token = ?"
    )
    .bind(&token)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

    let (connection_id, path, _pass_hash, expires_at) = row.ok_or_else(|| {
        AppError::NotFound("Shared link not found or expired".into())
    })?;

    // Check expiry
    if let Some(exp_str) = expires_at {
        if let Ok(exp_dt) = DateTime::parse_from_rfc3339(&exp_str) {
            if Utc::now() > exp_dt.with_timezone(&Utc) {
                return Err(AppError::Forbidden("This shared link has expired".into()));
            }
        }
    }

    let provider = state
        .get_provider(&connection_id)
        .await
        .ok_or_else(|| AppError::NotFound("Storage connection unavailable".into()))?;

    let vfs_path = crate::domain::VfsPath::new(&connection_id, &path);
    let stream = provider.read_stream(&vfs_path).await?;
    let body = axum::body::Body::from_stream(tokio_util::io::ReaderStream::new(stream));

    let file_name = path.split('/').last().unwrap_or("download").to_string();
    let mime = mime_guess::from_path(&file_name).first_or_octet_stream();

    let response = axum::response::Response::builder()
        .status(StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, mime.as_ref())
        .header(
            axum::http::header::CONTENT_DISPOSITION,
            format!("inline; filename=\"{}\"", file_name),
        )
        .body(body)
        .map_err(|e| AppError::Internal(e.into()))?;

    Ok(response)
}
