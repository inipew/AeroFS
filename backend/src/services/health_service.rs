use crate::db::DbPool;
use crate::errors::AppError;
use crate::runtime::TaskSupervisor;
use crate::state::RuntimeView;
use crate::vfs::registry::ProviderRegistry;
use std::path::PathBuf;
use std::sync::Arc;

const BACKGROUND_FAILURE_THRESHOLD: u32 = 3;

#[derive(Debug, Clone)]
pub struct ReadinessStatus {
    pub active_providers: usize,
    pub phase: &'static str,
}

#[derive(Clone)]
pub struct HealthService {
    db: DbPool,
    storage_root: PathBuf,
    registry: Arc<ProviderRegistry>,
    runtime: RuntimeView,
    supervisor: TaskSupervisor,
}

impl HealthService {
    pub fn new(
        db: DbPool,
        storage_root: PathBuf,
        registry: Arc<ProviderRegistry>,
        runtime: RuntimeView,
        supervisor: TaskSupervisor,
    ) -> Self {
        Self {
            db,
            storage_root,
            registry,
            runtime,
            supervisor,
        }
    }

    pub async fn readiness(&self) -> Result<ReadinessStatus, AppError> {
        let phase = self.runtime.phase();
        if !self.runtime.is_running() {
            return Err(AppError::ServiceUnavailable(format!(
                "Runtime phase is '{}'",
                phase.as_str()
            )));
        }

        let db_ok = sqlx::query("SELECT 1").fetch_one(&self.db).await.is_ok();
        let storage_ok = self.storage_root.exists();
        let degraded_tasks = self
            .supervisor
            .degraded_tasks(BACKGROUND_FAILURE_THRESHOLD);

        if !db_ok || !storage_ok || !degraded_tasks.is_empty() {
            let mut reasons = Vec::new();
            if !db_ok {
                reasons.push("Database query failed".to_string());
            }
            if !storage_ok {
                reasons.push("Storage root inaccessible".to_string());
            }
            for (name, health) in degraded_tasks {
                reasons.push(format!(
                    "Background task '{}' failed {} consecutive times{}",
                    name,
                    health.consecutive_failures,
                    health
                        .last_error
                        .as_deref()
                        .map(|error| format!(": {}", error))
                        .unwrap_or_default()
                ));
            }
            return Err(AppError::ServiceUnavailable(format!(
                "Readiness checks failed: {}",
                reasons.join(", ")
            )));
        }

        Ok(ReadinessStatus {
            active_providers: self.registry.list_ids().await.len(),
            phase: phase.as_str(),
        })
    }
}
