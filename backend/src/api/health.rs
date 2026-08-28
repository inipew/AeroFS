use crate::state::AppState;
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use std::time::Instant;
use utoipa::ToSchema;

static START_TIME: LazyLock<Instant> = LazyLock::new(Instant::now);

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LivenessResponse {
    pub status: String,
    pub uptime_seconds: u64,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ReadinessResponse {
    pub status: String,
    pub database: String,
    pub storage_root: String,
    pub active_providers: usize,
    pub version: String,
}

/// Fast liveness probe
pub async fn health_live() -> impl IntoResponse {
    Json(LivenessResponse {
        status: "alive".to_string(),
        uptime_seconds: START_TIME.elapsed().as_secs(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// Readiness probe checking DB, storage directory, and active providers
pub async fn health_ready(State(state): State<AppState>) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    // 1. Check Database connection
    let db_ok = sqlx::query("SELECT 1")
        .fetch_one(&state.db)
        .await
        .is_ok();

    // 2. Check Storage root
    let storage_ok = state.config.filesystem.default_local_root.exists();

    // 3. Check active providers
    let providers_count = state.providers.read().await.len();

    if db_ok && storage_ok {
        Ok(Json(ReadinessResponse {
            status: "ready".to_string(),
            database: "connected".to_string(),
            storage_root: "accessible".to_string(),
            active_providers: providers_count,
            version: env!("CARGO_PKG_VERSION").to_string(),
        }))
    } else {
        let mut reasons = Vec::new();
        if !db_ok {
            reasons.push("Database query failed");
        }
        if !storage_ok {
            reasons.push("Storage root inaccessible");
        }
        Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "status": "not_ready",
                "reasons": reasons,
                "version": env!("CARGO_PKG_VERSION")
            })),
        ))
    }
}

/// Legacy / backward compatible health endpoint
pub async fn health_check() -> impl IntoResponse {
    health_live().await
}
