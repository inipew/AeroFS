use crate::db::DbPool;
use crate::runtime::{ResourceBudget, ResourceBudgetMetrics, TaskSupervisor};
use crate::services::{MetadataCache, MetadataCacheMetrics};
use crate::vfs::registry::ProviderRegistry;
use serde::Serialize;
use sqlx::Row;
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

#[derive(Debug, Clone, Default, Serialize)]
pub struct DurableExecutionMetrics {
    /// Durable transfer rows that still belong to the execution lifecycle.
    pub transfer_active: usize,
    /// Durable background transfers waiting for a worker.
    pub transfer_queue_depth: usize,
    /// Durable transfers currently running or completing cancellation.
    pub transfer_running: usize,
    /// Durable sync jobs that have not reached completed/failed/conflict.
    pub sync_active: usize,
    /// Sync jobs currently doing scan/plan/reconcile/execute/verify work.
    pub sync_executing: usize,
    /// Sync jobs intentionally retained in the paused lifecycle state.
    pub sync_paused: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeMetricsSnapshot {
    pub active_supervised_tasks: usize,
    pub resource_budget: ResourceBudgetMetrics,
    pub providers: ProviderRuntimeMetrics,
    pub metadata_cache: MetadataCacheMetrics,
    pub database_pool: DatabasePoolMetrics,
    pub execution: DurableExecutionMetrics,
}

/// Lightweight operational snapshot used to verify that permits, leases, cache state, durable
/// execution state, and DB connections return toward their idle baseline after foreground work
/// completes.
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

    async fn durable_execution_metrics(&self) -> DurableExecutionMetrics {
        let transfer = sqlx::query(
            r#"
            SELECT
                COALESCE(SUM(CASE
                    WHEN status IN ('queued', 'running', 'cancellation_requested') THEN 1
                    ELSE 0
                END), 0) AS active,
                COALESCE(SUM(CASE WHEN status = 'queued' THEN 1 ELSE 0 END), 0) AS queued,
                COALESCE(SUM(CASE
                    WHEN status IN ('running', 'cancellation_requested') THEN 1
                    ELSE 0
                END), 0) AS running
            FROM transfer_jobs
            WHERE status IN ('queued', 'running', 'cancellation_requested')
            "#,
        )
        .fetch_one(&self.db)
        .await;

        let sync = sqlx::query(
            r#"
            SELECT
                COUNT(*) AS active,
                COALESCE(SUM(CASE
                    WHEN status IN ('scanning', 'planning', 'reconciling', 'executing', 'verifying')
                    THEN 1 ELSE 0
                END), 0) AS executing,
                COALESCE(SUM(CASE WHEN status = 'paused' THEN 1 ELSE 0 END), 0) AS paused
            FROM sync_jobs
            WHERE status IN (
                'created', 'scanning', 'planning', 'reconciling',
                'executing', 'verifying', 'paused'
            )
            "#,
        )
        .fetch_one(&self.db)
        .await;

        match (transfer, sync) {
            (Ok(transfer), Ok(sync)) => DurableExecutionMetrics {
                transfer_active: transfer.get::<i64, _>("active").max(0) as usize,
                transfer_queue_depth: transfer.get::<i64, _>("queued").max(0) as usize,
                transfer_running: transfer.get::<i64, _>("running").max(0) as usize,
                sync_active: sync.get::<i64, _>("active").max(0) as usize,
                sync_executing: sync.get::<i64, _>("executing").max(0) as usize,
                sync_paused: sync.get::<i64, _>("paused").max(0) as usize,
            },
            (transfer_result, sync_result) => {
                if let Err(error) = transfer_result {
                    tracing::warn!(%error, "runtime metrics: failed to query transfer execution state");
                }
                if let Err(error) = sync_result {
                    tracing::warn!(%error, "runtime metrics: failed to query sync execution state");
                }
                DurableExecutionMetrics::default()
            }
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
        let execution = self.durable_execution_metrics().await;

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
            execution,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[tokio::test]
    async fn snapshot_counts_only_nonterminal_execution_state() {
        let db = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&db).await.unwrap();

        let now = Utc::now().to_rfc3339();
        for (id, status) in [
            ("t-queued", "queued"),
            ("t-running", "running"),
            ("t-cancelling", "cancellation_requested"),
            ("t-completed", "completed"),
        ] {
            sqlx::query(
                r#"
                INSERT INTO transfer_jobs (
                    id, name, transfer_type, source_connection_id, source_path,
                    destination_connection_id, destination_path, status, created_at, updated_at
                ) VALUES (?, ?, 'copy', 'local', '/src', 'local', '/dst', ?, ?, ?)
                "#,
            )
            .bind(id)
            .bind(id)
            .bind(status)
            .bind(&now)
            .bind(&now)
            .execute(&db)
            .await
            .unwrap();
        }

        for (id, status) in [
            ("s-created", "created"),
            ("s-executing", "executing"),
            ("s-paused", "paused"),
            ("s-completed", "completed"),
        ] {
            sqlx::query(
                r#"
                INSERT INTO sync_jobs (
                    id, user_id, source_connection_id, source_path,
                    destination_connection_id, destination_path, status, strategy,
                    created_at, updated_at
                ) VALUES (?, 'user-1', 'local', '/src', 'local', '/dst', ?, 'source_wins', ?, ?)
                "#,
            )
            .bind(id)
            .bind(status)
            .bind(&now)
            .bind(&now)
            .execute(&db)
            .await
            .unwrap();
        }

        let collector = RuntimeMetricsCollector::new(
            Arc::new(ResourceBudget::default()),
            TaskSupervisor::new(),
            Arc::new(ProviderRegistry::new()),
            Arc::new(MetadataCache::default()),
            db,
        );
        let execution = collector.snapshot().await.execution;

        assert_eq!(execution.transfer_active, 3);
        assert_eq!(execution.transfer_queue_depth, 1);
        assert_eq!(execution.transfer_running, 2);
        assert_eq!(execution.sync_active, 3);
        assert_eq!(execution.sync_executing, 1);
        assert_eq!(execution.sync_paused, 1);
    }
}
