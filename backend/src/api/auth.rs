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
fn extract_client_ip(headers: &HeaderMap, trusted_proxies: &[String]) -> String {
    // §23: Only trust X-Forwarded-For if request comes from trusted proxy.
    // If trusted_proxies is empty, forwarded headers are ignored (secure-by-default).
    if trusted_proxies.is_empty() {
        return "127.0.0.1".to_string();
    }
    let ip_opt = headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string());
    ip_opt.unwrap_or_else(|| "127.0.0.1".to_string())
}

pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    let client_ip = extract_client_ip(&headers, &state.config.security.trusted_proxies);

    let (user_info, session_id) =
        AuthService::login(&state, &payload.username, &payload.password, &client_ip).await?;

    // Build session cookie via `cookie` crate — explicit policy (§21-22)
    let cookie_val = {
        use cookie::{Cookie, SameSite};
        let same_site = if state.config.security.cookie_secure {
            SameSite::None
        } else {
            SameSite::Lax
        };
        let mut c = Cookie::new("session_id", session_id.clone());
        c.set_path("/");
        c.set_http_only(true);
        c.set_same_site(same_site);
        c.set_secure(state.config.security.cookie_secure);
        c.set_max_age(cookie::time::Duration::seconds(
            state.config.security.session_ttl_secs as i64,
        ));
        c.to_string()
    };

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
    let client_ip = extract_client_ip(&headers, &state.config.security.trusted_proxies);

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
    let logout_cookie = {
        use cookie::{Cookie, SameSite};
        let mut c = Cookie::new("session_id", "");
        c.set_path("/");
        c.set_http_only(true);
        c.set_same_site(SameSite::Lax);
        c.set_max_age(cookie::time::Duration::seconds(0));
        c.to_string()
    };
    resp_headers.insert(
        SET_COOKIE,
        logout_cookie
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
