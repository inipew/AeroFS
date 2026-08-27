use crate::auth::password::verify_password;
use crate::auth::session::{create_session, delete_session, UserInfo};
use crate::auth::AuthenticatedUser;
use crate::errors::{AppError, AuthError};
use crate::state::AppState;
use axum::{
    extract::State,
    http::{header::SET_COOKIE, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthResponse {
    pub user: UserInfo,
    pub session_id: String,
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = AuthResponse),
        (status = 401, description = "Invalid credentials")
    )
)]
pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    let row: Option<(String, String, String, i64)> = sqlx::query_as(
        "SELECT id, username, password_hash, is_admin FROM users WHERE username = ?",
    )
    .bind(&payload.username)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| anyhow::anyhow!("Database query error: {}", e))?;

    let (user_id, username, password_hash, is_admin) = match row {
        Some(r) => r,
        None => return Err(AppError::Auth(AuthError::InvalidCredentials)),
    };

    if !verify_password(&payload.password, &password_hash) {
        return Err(AppError::Auth(AuthError::InvalidCredentials));
    }

    let session_id = create_session(&state.db, &user_id, state.config.security.session_ttl_secs).await?;

    let cookie_val = format!(
        "session_id={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
        session_id, state.config.security.session_ttl_secs
    );

    let mut headers = HeaderMap::new();
    headers.insert(SET_COOKIE, cookie_val.parse().unwrap());

    let user_info = UserInfo {
        id: user_id,
        username,
        is_admin: is_admin != 0,
    };

    Ok((
        StatusCode::OK,
        headers,
        Json(AuthResponse {
            user: user_info,
            session_id,
        }),
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/logout",
    responses(
        (status = 200, description = "Logged out successfully")
    )
)]
pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    if let Some(cookie_header) = headers.get(axum::http::header::COOKIE) {
        let cookie_str = cookie_header.to_str().unwrap_or_default();
        if let Some(session_id) = cookie_str.split(';').find_map(|c| {
            let trimmed = c.trim();
            trimmed.strip_prefix("session_id=")
        }) {
            let _ = delete_session(&state.db, session_id).await;
        }
    }

    let mut resp_headers = HeaderMap::new();
    resp_headers.insert(
        SET_COOKIE,
        "session_id=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"
            .parse()
            .unwrap(),
    );

    Ok((
        StatusCode::OK,
        resp_headers,
        Json(serde_json::json!({ "message": "Logged out" })),
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/me",
    responses(
        (status = 200, description = "Current authenticated user", body = UserInfo),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn me(
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    Ok(Json(user))
}
