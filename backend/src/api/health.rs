use crate::errors::{AppError, ErrorResponse};
use crate::state::HealthState;
use axum::{extract::State, response::IntoResponse, Json};
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
    pub phase: String,
    pub version: String,
}

/// Fast liveness probe — always 200 as long as the process is alive
#[utoipa::path(
    get,
    path = "/api/v1/health/live",
    responses(
        (status = 200, description = "Process is alive", body = LivenessResponse)
    ),
    tag = "health"
)]
pub async fn health_live() -> impl IntoResponse {
    Json(LivenessResponse {
        status: "alive".to_string(),
        uptime_seconds: START_TIME.elapsed().as_secs(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// Readiness probe: 503 when starting or shutting down, 200 when fully running
#[utoipa::path(
    get,
    path = "/api/v1/health/ready",
    responses(
        (status = 200, description = "Service is ready to handle traffic", body = ReadinessResponse),
        (status = 503, description = "Service is unavailable", body = ErrorResponse)
    ),
    tag = "health"
)]
pub async fn health_ready(State(state): State<HealthState>) -> Result<impl IntoResponse, AppError> {
    let status = state.service.readiness().await?;
    Ok(Json(ReadinessResponse {
        status: "ready".to_string(),
        database: "connected".to_string(),
        storage_root: "accessible".to_string(),
        active_providers: status.active_providers,
        phase: status.phase.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    }))
}

/// Legacy / backward compatible health endpoint
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Legacy health check", body = LivenessResponse)
    ),
    tag = "health"
)]
pub async fn health_check() -> impl IntoResponse {
    health_live().await
}
