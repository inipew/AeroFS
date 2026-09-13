use crate::db::DbPool;
use crate::ports::health::{BackgroundTaskDegradation, ReadinessProbe, ReadinessSignals};
use crate::runtime::TaskSupervisor;
use crate::state::RuntimeView;
use crate::vfs::registry::ProviderRegistry;
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct RuntimeReadinessProbe {
    db: DbPool,
    storage_root: PathBuf,
    registry: Arc<ProviderRegistry>,
    runtime: RuntimeView,
    supervisor: TaskSupervisor,
}

impl RuntimeReadinessProbe {
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
}

#[async_trait]
impl ReadinessProbe for RuntimeReadinessProbe {
    async fn signals(&self, background_failure_threshold: u32) -> ReadinessSignals {
        let phase = self.runtime.phase();
        let database_ok = sqlx::query("SELECT 1").fetch_one(&self.db).await.is_ok();
        let storage_ok = self.storage_root.exists();
        let degraded_tasks = self
            .supervisor
            .readiness_degraded_tasks(background_failure_threshold)
            .into_iter()
            .map(|(name, health)| BackgroundTaskDegradation {
                name,
                consecutive_failures: health.consecutive_failures,
                last_error: health.last_error,
                restart_exhausted: health.restart_exhausted,
                restart_count: health.restart_count,
            })
            .collect();

        ReadinessSignals {
            runtime_running: self.runtime.is_running(),
            phase: phase.as_str(),
            database_ok,
            storage_ok,
            active_providers: self.registry.list_ids().await.len(),
            degraded_tasks,
        }
    }
}
