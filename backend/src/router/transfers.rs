//! Transfers router sub-module (§45)

use crate::api::transfers as api_transfers;
use crate::state::AppState;
use axum::{
    routing::{get, post},
    Router,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/",
            get(api_transfers::list_transfers).post(api_transfers::create_transfer),
        )
        .route("/{id}/cancel", post(api_transfers::cancel_transfer))
        .route("/{id}/retry", post(api_transfers::retry_transfer))
        .route("/{id}/dismiss", post(api_transfers::dismiss_transfer))
        .route(
            "/clear-finished",
            post(api_transfers::clear_finished_transfers),
        )
}
