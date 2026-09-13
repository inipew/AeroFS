use crate::errors::AppError;
use crate::ports::health::ReadinessProbe;
use std::sync::Arc;

const BACKGROUND_FAILURE_THRESHOLD: u32 = 3;

#[derive(Debug, Clone)]
pub struct ReadinessStatus {
    pub active_providers: usize,
    pub phase: &'static str,
}

#[derive(Clone)]
pub struct HealthService {
    probe: Arc<dyn ReadinessProbe>,
}

impl HealthService {
    pub fn new(probe: Arc<dyn ReadinessProbe>) -> Self {
        Self { probe }
    }

    pub async fn readiness(&self) -> Result<ReadinessStatus, AppError> {
        let signals = self.probe.signals(BACKGROUND_FAILURE_THRESHOLD).await;
        if !signals.runtime_running {
            return Err(AppError::ServiceUnavailable(format!(
                "Runtime phase is '{}'",
                signals.phase
            )));
        }

        if !signals.database_ok || !signals.storage_ok || !signals.degraded_tasks.is_empty() {
            let mut reasons = Vec::new();
            if !signals.database_ok {
                reasons.push("Database query failed".to_string());
            }
            if !signals.storage_ok {
                reasons.push("Storage root inaccessible".to_string());
            }
            for health in signals.degraded_tasks {
                let restart_suffix = if health.restart_exhausted {
                    format!(
                        "; restart budget exhausted after {} restarts",
                        health.restart_count
                    )
                } else {
                    String::new()
                };
                reasons.push(format!(
                    "Critical background task '{}' failed {} consecutive times{}{}",
                    health.name,
                    health.consecutive_failures,
                    health
                        .last_error
                        .as_deref()
                        .map(|error| format!(": {}", error))
                        .unwrap_or_default(),
                    restart_suffix,
                ));
            }
            return Err(AppError::ServiceUnavailable(format!(
                "Readiness checks failed: {}",
                reasons.join(", ")
            )));
        }

        Ok(ReadinessStatus {
            active_providers: signals.active_providers,
            phase: signals.phase,
        })
    }
}
