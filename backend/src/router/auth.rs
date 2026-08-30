//! Auth router sub-module (§45)

use crate::api::auth as api_auth;
use crate::state::AppState;
use axum::{
    routing::{get, post},
    Router,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/login", post(api_auth::login))
        .route("/logout", post(api_auth::logout))
        .route("/me", get(api_auth::me))
}
