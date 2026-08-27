use crate::auth::password::verify_password;
use crate::auth::permissions::{check_permission, PermissionAction};
use crate::auth::AuthenticatedUser;
use crate::domain::VfsPath;
use crate::errors::{AppError, VfsError};
use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
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

#[derive(Debug, Deserialize)]
pub struct PublicShareQuery {
    pub password: Option<String>,
}

/// List shares with strict user ownership filter (Admins can view all)
pub async fn list_shares(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    let rows: Vec<(
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        String,
    )> = if user.is_admin {
        sqlx::query_as(
            "SELECT id, connection_id, path, share_token, password_hash, expires_at, created_at
             FROM shares
             ORDER BY created_at DESC",
        )
        .fetch_all(&state.db)
        .await
        .map_err(|e| anyhow::anyhow!("Database error: {}", e))?
    } else {
        sqlx::query_as(
            "SELECT id, connection_id, path, share_token, password_hash, expires_at, created_at
             FROM shares
             WHERE created_by = ?
             ORDER BY created_at DESC",
        )
        .bind(&user.username)
        .fetch_all(&state.db)
        .await
        .map_err(|e| anyhow::anyhow!("Database error: {}", e))?
    };

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

/// Create a new share link with connection permission and file existence verification
pub async fn create_share(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<CreateShareRequest>,
) -> Result<impl IntoResponse, AppError> {
    // 1. Authorize user has READ permission on connection
    check_permission(
        &state.db,
        &user,
        &payload.connection_id,
        PermissionAction::Read,
    )
    .await?;

    // 2. Verify target file actually exists in provider
    let provider = state
        .get_provider(&payload.connection_id)
        .await
        .ok_or_else(|| {
            VfsError::ConnectionError(format!("Connection '{}' not found", payload.connection_id))
        })?;

    let vfs_path = VfsPath::new(&payload.connection_id, &payload.path);
    provider.stat(&vfs_path).await?;

    let id = Uuid::new_v4().to_string();
    let share_token = format!("sh_{}", &Uuid::new_v4().to_string().replace('-', "")[..16]);
    let now = Utc::now();
    let now_str = now.to_rfc3339();

    let expires_at_str = payload
        .expires_in_hours
        .map(|h| (now + Duration::hours(h)).to_rfc3339());

    let password_hash = payload.password.and_then(|p| {
        if p.trim().is_empty() {
            None
        } else {
            crate::auth::password::hash_password(&p).ok()
        }
    });

    sqlx::query(
        "INSERT INTO shares (id, connection_id, path, share_token, password_hash, expires_at, created_at, created_by)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
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

/// Delete a share link with ownership verification
pub async fn delete_share(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let res = if user.is_admin {
        sqlx::query("DELETE FROM shares WHERE id = ?")
            .bind(&id)
            .execute(&state.db)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to delete share: {}", e))?
    } else {
        sqlx::query("DELETE FROM shares WHERE id = ? AND created_by = ?")
            .bind(&id)
            .bind(&user.username)
            .execute(&state.db)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to delete share: {}", e))?
    };

    if res.rows_affected() == 0 {
        return Err(AppError::Forbidden(
            "Share not found or you do not have permission to delete it".into(),
        ));
    }

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Share link revoked successfully"
    })))
}

/// Public download / view of shared file with password verification
pub async fn public_get_share(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Query(query): Query<PublicShareQuery>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let row: Option<(String, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT connection_id, path, password_hash, expires_at FROM shares WHERE share_token = ?",
    )
    .bind(&token)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

    let (connection_id, path, pass_hash, expires_at) = row.ok_or_else(|| {
        AppError::NotFound("Shared link not found or expired".into())
    })?;

    // 1. Check expiration
    if let Some(exp_str) = expires_at {
        if let Ok(exp_dt) = DateTime::parse_from_rfc3339(&exp_str) {
            if Utc::now() > exp_dt.with_timezone(&Utc) {
                return Err(AppError::Forbidden("This shared link has expired".into()));
            }
        }
    }

    // 2. Strict Password Verification
    if let Some(expected_hash) = pass_hash {
        let provided_password = query.password.or_else(|| {
            headers
                .get("X-Share-Password")
                .and_then(|h| h.to_str().ok().map(|s| s.to_string()))
        });

        match provided_password {
            Some(pwd) => {
                if !verify_password(&pwd, &expected_hash) {
                    return Err(AppError::Unauthorized(
                        "Invalid password for shared link".into(),
                    ));
                }
            }
            None => {
                return Err(AppError::Unauthorized(
                    "Password required to access this shared link".into(),
                ));
            }
        }
    }

    let provider = state
        .get_provider(&connection_id)
        .await
        .ok_or_else(|| AppError::NotFound("Storage connection unavailable".into()))?;

    let vfs_path = VfsPath::new(&connection_id, &path);
    let stream = provider.read_stream(&vfs_path).await?;
    let body = axum::body::Body::from_stream(tokio_util::io::ReaderStream::new(stream));

    let file_name = path.split('/').last().unwrap_or("download").to_string();
    let mime = mime_guess::from_path(&file_name).first_or_octet_stream();

    // 3. Stored XSS / Malicious File Protection:
    // Enforce 'attachment' for HTML, Javascript, and executable MIME types
    let is_dangerous_inline = mime.as_ref().starts_with("text/html")
        || mime.as_ref().starts_with("application/javascript")
        || mime.as_ref().starts_with("text/javascript")
        || mime.as_ref().starts_with("image/svg+xml");

    let disposition = if is_dangerous_inline {
        format!("attachment; filename=\"{}\"", file_name.replace('"', ""))
    } else {
        format!("inline; filename=\"{}\"", file_name.replace('"', ""))
    };

    let response = axum::response::Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime.as_ref())
        .header(header::CONTENT_DISPOSITION, disposition)
        .body(body)
        .map_err(|e| AppError::Internal(e.into()))?;

    Ok(response)
}
