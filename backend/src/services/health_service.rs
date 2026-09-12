use crate::db::DbPool;
use crate::errors::AppError;
use crate::state::RuntimeView;
use crate::vfs::registry::ProviderRegistry;
use std::path::PathBuf;
use std::sync::Arc;

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
}

impl HealthService {
    pub fn new(
        db: DbPool,
        storage_root: PathBuf,
        registry: Arc<ProviderRegistry>,
        runtime: RuntimeView,
    ) -> Self {
        Self {
            db,
            storage_root,
            registry,
            runtime,
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
        if !db_ok || !storage_ok {
            let mut reasons = Vec::new();
            if !db_ok {
                reasons.push("Database query failed");
            }
            if !storage_ok {
                reasons.push("Storage root inaccessible");
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
