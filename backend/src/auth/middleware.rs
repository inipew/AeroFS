use crate::auth::session::UserInfo;
use crate::errors::{AppError, AuthError};
use crate::state::AuthState;
use axum::{
    extract::{FromRef, FromRequestParts},
    http::{header, request::Parts},
};
use std::ops::Deref;

/// Axum extractor for authenticated user sessions — invariant: user is authenticated.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser(pub UserInfo);

impl AuthenticatedUser {
    pub fn id(&self) -> &str {
        &self.0.id
    }
    pub fn username(&self) -> &str {
        &self.0.username
    }
    pub fn is_admin(&self) -> bool {
        self.0.is_admin
    }
    pub fn user_info(&self) -> &UserInfo {
        &self.0
    }
}

impl Deref for AuthenticatedUser {
    type Target = UserInfo;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<S> FromRequestParts<S> for AuthenticatedUser
where
    AuthState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let auth = AuthState::from_ref(state);

        if let Some(cookie_header) = parts.headers.get(header::COOKIE) {
            if let Ok(cookie_str) = cookie_header.to_str() {
                for cookie in cookie_str.split(';') {
                    let mut parts = cookie.trim().splitn(2, '=');
                    if let (Some(name), Some(val)) = (parts.next(), parts.next()) {
                        if name == "session_id" {
                            let user = auth
                                .service
                                .validate_session(val)
                                .await?
                                .ok_or(AuthError::SessionExpired)?;
                            return Ok(AuthenticatedUser(user));
                        }
                    }
                }
            }
        }

        if let Some(auth_header) = parts.headers.get(header::AUTHORIZATION) {
            if let Ok(auth_str) = auth_header.to_str() {
                if let Some(token) = auth_str.strip_prefix("Bearer ") {
                    let user = auth
                        .service
                        .validate_session(token)
                        .await?
                        .ok_or(AuthError::SessionExpired)?;
                    return Ok(AuthenticatedUser(user));
                }
            }
        }

        let is_ws = parts.uri.path() == "/api/v1/ws";
        if is_ws {
            if let Some(query) = parts.uri.query() {
                for pair in query.split('&') {
                    let mut kv = pair.splitn(2, '=');
                    if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
                        if k == "session_id" || k == "token" {
                            let decoded = urlencoding::decode(v).unwrap_or_else(|_| v.into());
                            if let Ok(Some(user)) = auth.service.validate_session(&decoded).await {
                                return Ok(AuthenticatedUser(user));
                            }
                        }
                    }
                }
            }
        }

        Err(AuthError::SessionExpired.into())
    }
}
