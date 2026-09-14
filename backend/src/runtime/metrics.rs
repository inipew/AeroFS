use crate::db::DbPool;
use crate::runtime::{ResourceBudget, ResourceBudgetMetrics, TaskSupervisor};
use crate::services::{MetadataCache, MetadataCacheMetrics};
use crate::vfs::registry::ProviderRegistry;
use serde::Serialize;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize)]
pub struct ProviderRuntimeMetrics {
    pub registered: usize,
    pub reclaimable: usize,
    pub active_leases: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct DatabasePoolMetrics {
    pub open_connections: u32,
    pub idle_connections: usize,
    pub in_use_connections: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeMetricsSnapshot {
    pub active_supervised_tasks: usize,
    pub resource_budget: ResourceBudgetMetrics,
    pub providers: ProviderRuntimeMetrics,
    pub metadata_cache: MetadataCacheMetrics,
    pub database_pool: DatabasePoolMetrics,
}

/// Lightweight operational snapshot used to verify that permits, leases, cache state and DB
/// connections return toward their idle baseline after foreground work completes.
#[derive(Clone)]
pub struct RuntimeMetricsCollector {
    resource_budget: Arc<ResourceBudget>,
    supervisor: TaskSupervisor,
    registry: Arc<ProviderRegistry>,
    metadata_cache: Arc<MetadataCache>,
    db: DbPool,
}

impl RuntimeMetricsCollector {
    pub fn new(
        resource_budget: Arc<ResourceBudget>,
        supervisor: TaskSupervisor,
        registry: Arc<ProviderRegistry>,
        metadata_cache: Arc<MetadataCache>,
        db: DbPool,
    ) -> Self {
        Self {
            resource_budget,
            supervisor,
            registry,
            metadata_cache,
            db,
        }
    }

    pub async fn snapshot(&self) -> RuntimeMetricsSnapshot {
        let runtimes = self.registry.runtimes_map();
        let runtimes = runtimes.read().await;
        let registered = runtimes.len();
        let reclaimable = runtimes
            .values()
            .filter(|runtime| runtime.is_reclaimable())
            .count();
        let active_leases = runtimes
            .values()
            .map(|runtime| runtime.active_leases_count())
            .sum();
        drop(runtimes);

        let open_connections = self.db.size();
        let idle_connections = self.db.num_idle();

        RuntimeMetricsSnapshot {
            active_supervised_tasks: self.supervisor.active_tasks(),
            resource_budget: self.resource_budget.metrics(),
            providers: ProviderRuntimeMetrics {
                registered,
                reclaimable,
                active_leases,
            },
            metadata_cache: self.metadata_cache.metrics().await,
            database_pool: DatabasePoolMetrics {
                open_connections,
                idle_connections,
                in_use_connections: (open_connections as usize).saturating_sub(idle_connections),
            },
        }
    }
}
