use crate::auth::session::UserInfo;
use crate::auth::AuthenticatedUser;
use crate::errors::AppError;
use crate::services::AuthService;
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
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = AuthResponse),
        (status = 401, description = "Invalid credentials"),
        (status = 429, description = "Too many failed attempts")
    )
)]
pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    let client_ip = headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "127.0.0.1".to_string());

    let (user_info, session_id) =
        AuthService::login(&state, &payload.username, &payload.password, &client_ip).await?;

    // Determine whether to set the Secure flag on the session cookie.
    // Use the explicit config field (cookie_secure, default false).
    // On Android/LAN plain-HTTP deployments keep this false (the default).
    let secure_flag = if state.config.security.cookie_secure {
        "; Secure"
    } else {
        ""
    };

    // SameSite=Lax works for same-site navigation but breaks cross-origin requests
    // (e.g. WebView on Android hitting a LAN IP). Use SameSite=Lax only when Secure
    // is set (HTTPS); otherwise fall back to SameSite=Lax without Secure for HTTP LAN.
    let same_site = if secure_flag.is_empty() {
        "SameSite=Lax"
    } else {
        "SameSite=None"
    };

    let cookie_val = format!(
        "session_id={}; Path=/; HttpOnly; {}; Max-Age={}{}",
        session_id, same_site, state.config.security.session_ttl_secs, secure_flag
    );

    let mut resp_headers = HeaderMap::new();
    resp_headers.insert(
        SET_COOKIE,
        cookie_val
            .parse()
            .map_err(|e| anyhow::anyhow!("Cookie parse error: {}", e))?,
    );

    Ok((
        StatusCode::OK,
        resp_headers,
        Json(AuthResponse { user: user_info }),
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/logout",
    responses(
        (status = 200, description = "Logout successful")
    )
)]
pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let client_ip = headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "127.0.0.1".to_string());

    if let Some(cookie_header) = headers.get(axum::http::header::COOKIE) {
        let cookie_str = cookie_header.to_str().unwrap_or_default();
        if let Some(session_id) = cookie_str.split(';').find_map(|c| {
            let trimmed = c.trim();
            trimmed.strip_prefix("session_id=")
        }) {
            let _ = AuthService::logout(&state, session_id, None, &client_ip).await;
        }
    }

    let mut resp_headers = HeaderMap::new();
    resp_headers.insert(
        SET_COOKIE,
        "session_id=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"
            .parse()
            .map_err(|e| anyhow::anyhow!("Cookie parse error: {}", e))?,
    );

    Ok((
        StatusCode::OK,
        resp_headers,
        Json(serde_json::json!({ "success": true, "message": "Logged out successfully" })),
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/me",
    responses(
        (status = 200, description = "Current authenticated user profile", body = UserInfo),
        (status = 401, description = "Not authenticated")
    )
)]
pub async fn me(AuthenticatedUser(user): AuthenticatedUser) -> Result<impl IntoResponse, AppError> {
    Ok(Json(user))
}
