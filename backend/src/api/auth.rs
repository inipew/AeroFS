use crate::api::extractors::Json;
use crate::auth::session::UserInfo;
use crate::auth::AuthenticatedUser;
use crate::errors::{AppError, ErrorResponse};
use crate::state::AuthState;
use axum::{
    extract::State,
    http::{header::SET_COOKIE, HeaderMap, StatusCode},
    response::IntoResponse,
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

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LogoutResponse {
    pub success: bool,
    pub message: String,
}

fn extract_client_ip(headers: &HeaderMap, trusted_proxies: &[String]) -> String {
    if trusted_proxies.is_empty() {
        return "127.0.0.1".to_string();
    }
    headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "127.0.0.1".to_string())
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = AuthResponse),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Invalid credentials", body = ErrorResponse),
        (status = 429, description = "Too many failed attempts", body = ErrorResponse)
    ),
    tag = "auth"
)]
pub async fn login(
    State(state): State<AuthState>,
    headers: HeaderMap,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    let client_ip = extract_client_ip(&headers, state.service.trusted_proxies());
    let (user_info, session_id) = state
        .service
        .login(&payload.username, &payload.password, &client_ip)
        .await?;

    let cookie_val = {
        use cookie::{Cookie, SameSite};
        let same_site = if state.service.cookie_secure() {
            SameSite::None
        } else {
            SameSite::Lax
        };
        let mut c = Cookie::new("session_id", session_id);
        c.set_path("/");
        c.set_http_only(true);
        c.set_same_site(same_site);
        c.set_secure(state.service.cookie_secure());
        c.set_max_age(cookie::time::Duration::seconds(
            state.service.session_ttl_secs() as i64,
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
        axum::Json(AuthResponse { user: user_info }),
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/logout",
    responses(
        (status = 200, description = "Logout successful", body = LogoutResponse)
    ),
    tag = "auth"
)]
pub async fn logout(
    State(state): State<AuthState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let client_ip = extract_client_ip(&headers, state.service.trusted_proxies());

    if let Some(cookie_header) = headers.get(axum::http::header::COOKIE) {
        let cookie_str = cookie_header.to_str().unwrap_or_default();
        if let Some(session_id) = cookie_str.split(';').find_map(|c| {
            c.trim().strip_prefix("session_id=")
        }) {
            let _ = state.service.logout(session_id, None, &client_ip).await;
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
        axum::Json(LogoutResponse {
            success: true,
            message: "Logged out successfully".to_string(),
        }),
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/me",
    responses(
        (status = 200, description = "Current authenticated user profile", body = UserInfo),
        (status = 401, description = "Not authenticated", body = ErrorResponse)
    ),
    security(("CookieAuth" = []), ("BearerAuth" = [])),
    tag = "auth"
)]
pub async fn me(AuthenticatedUser(user): AuthenticatedUser) -> Result<impl IntoResponse, AppError> {
    Ok(axum::Json(user))
}
