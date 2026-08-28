use crate::state::AppState;
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct HealthStatus {
    pub status: String,
    pub version: String,
    pub uptime_secs: u64,
    pub active_providers: usize,
}

pub struct HealthService;

impl HealthService {
    pub async fn check_health(state: &AppState) -> HealthStatus {
        let providers_count = state.registry.list_ids().await.len();
        HealthStatus {
            status: "healthy".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_secs: 0,
            active_providers: providers_count,
        }
    }
}
