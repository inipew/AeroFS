use crate::support::architecture_source as source;

#[test]
fn health_http_uses_only_health_capability() {
    let state = source("src/state.rs");
    assert!(state.contains("pub struct HealthState"));
    assert!(state.contains("impl_from_ref!(HealthState, health)"));
    assert!(state.contains("pub struct RuntimeView"));

    let api = source("src/api/health.rs");
    assert!(api.contains("State<HealthState>"));
    for forbidden in [
        "State<AppState>",
        "state.db",
        "state.config",
        "state.registry",
        "state.runtime",
        "sqlx::",
    ] {
        assert!(!api.contains(forbidden), "health HTTP adapter leaked {forbidden}");
    }
}

#[test]
fn health_service_depends_on_readiness_port_and_infrastructure_owns_runtime_details() {
    let service = source("src/services/health_service.rs");
    assert!(service.contains("Arc<dyn ReadinessProbe>"));
    assert!(service.contains("pub async fn readiness(&self)"));
    for forbidden in [
        "AppState",
        "DbPool",
        "ProviderRegistry",
        "RuntimeView",
        "TaskSupervisor",
        "sqlx::",
    ] {
        assert!(!service.contains(forbidden), "health service leaked {forbidden}");
    }

    let adapter = source("src/infrastructure/health.rs");
    for expected in ["DbPool", "ProviderRegistry", "RuntimeView", "TaskSupervisor"] {
        assert!(adapter.contains(expected), "health adapter missing {expected}");
    }
}
