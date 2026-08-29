use crate::db::DbPool;
use crate::runtime::TaskSupervisor;
use crate::sync::conflict::ConflictResolver;
use crate::sync::diff::ManifestDiffer;
use crate::sync::models::{FileManifest, SyncJob, SyncOpKind, SyncStatus, SyncStrategy};
use crate::transfer::TransferManager;
use chrono::{DateTime, Utc};
use sqlx::Row;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// High-level synchronization manager coordinating manifests, reconciliation, and transfer execution.
#[derive(Clone)]
pub struct SyncManager {
    db: DbPool,
    transfer_manager: TransferManager,
    supervisor: TaskSupervisor,
    jobs: Arc<RwLock<HashMap<String, SyncJob>>>,
}

impl SyncManager {
    pub fn new(
        db: DbPool,
        transfer_manager: TransferManager,
        supervisor: TaskSupervisor,
    ) -> Self {
        Self {
            db,
            transfer_manager,
            supervisor,
            jobs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn supervisor(&self) -> &TaskSupervisor {
        &self.supervisor
    }

    /// Create and persist a new synchronization job.
    pub async fn create_job(
        &self,
        user_id: impl Into<String>,
        source_connection_id: impl Into<String>,
        source_path: impl Into<String>,
        destination_connection_id: impl Into<String>,
        destination_path: impl Into<String>,
        strategy: SyncStrategy,
    ) -> anyhow::Result<SyncJob> {
        let id = Uuid::new_v4().to_string();
        let user_id_str = user_id.into();
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let src_conn = source_connection_id.into();
        let src_path = source_path.into();
        let dst_conn = destination_connection_id.into();
        let dst_path = destination_path.into();

        sqlx::query(
            "INSERT INTO sync_jobs (id, user_id, source_connection_id, source_path, destination_connection_id, destination_path, status, strategy, total_files, synced_files, conflict_files, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0, 0, 0, ?, ?)",
        )
        .bind(&id)
        .bind(&user_id_str)
        .bind(&src_conn)
        .bind(&src_path)
        .bind(&dst_conn)
        .bind(&dst_path)
        .bind(SyncStatus::Created.as_str())
        .bind(strategy.as_str())
        .bind(&now_str)
        .bind(&now_str)
        .execute(&self.db)
        .await?;

        let job = SyncJob {
            id: id.clone(),
            user_id: user_id_str,
            source_connection_id: src_conn,
            source_path: src_path,
            destination_connection_id: dst_conn,
            destination_path: dst_path,
            status: SyncStatus::Created,
            strategy,
            total_files: 0,
            synced_files: 0,
            conflict_files: 0,
            created_at: now,
            updated_at: now,
        };

        {
            let mut jobs = self.jobs.write().await;
            jobs.insert(id, job.clone());
        }

        Ok(job)
    }

    /// Reconcile source and destination manifests and submit necessary file transfers.
    pub async fn execute_reconciliation(
        &self,
        job_id: &str,
        source_manifest: Vec<FileManifest>,
        dest_manifest: Vec<FileManifest>,
    ) -> anyhow::Result<usize> {
        let job_opt = {
            let jobs = self.jobs.read().await;
            jobs.get(job_id).cloned()
        };

        let job = match job_opt {
            Some(j) => j,
            None => return Err(anyhow::anyhow!("Sync job not found: {}", job_id)),
        };

        let ops = ManifestDiffer::diff(&source_manifest, &dest_manifest, job.strategy);
        let mut transfers_submitted = 0;
        let mut conflicts_detected = 0;

        for op in ops {
            match op.kind {
                SyncOpKind::Create | SyncOpKind::Update => {
                    let src_full = format!("{}/{}", job.source_path.trim_end_matches('/'), op.relative_path.trim_start_matches('/'));
                    let dst_full = format!("{}/{}", job.destination_path.trim_end_matches('/'), op.relative_path.trim_start_matches('/'));

                    let _ = self.transfer_manager.submit_job(
                        Some(job.user_id.clone()),
                        format!("Sync: {}", op.relative_path),
                        crate::transfer::TransferType::Copy,
                        job.source_connection_id.clone(),
                        src_full,
                        job.destination_connection_id.clone(),
                        dst_full,
                    ).await.map_err(|e| anyhow::anyhow!("{}", e))?;

                    transfers_submitted += 1;
                }
                SyncOpKind::Conflict => {
                    conflicts_detected += 1;
                    if job.strategy == SyncStrategy::KeepBoth {
                        let conflict_name = ConflictResolver::generate_conflict_filename(&op.relative_path);
                        let src_full = format!("{}/{}", job.source_path.trim_end_matches('/'), op.relative_path.trim_start_matches('/'));
                        let dst_full = format!("{}/{}", job.destination_path.trim_end_matches('/'), conflict_name.trim_start_matches('/'));

                        let _ = self.transfer_manager.submit_job(
                            Some(job.user_id.clone()),
                            format!("Sync Conflict: {}", conflict_name),
                            crate::transfer::TransferType::Copy,
                            job.source_connection_id.clone(),
                            src_full,
                            job.destination_connection_id.clone(),
                            dst_full,
                        ).await.map_err(|e| anyhow::anyhow!("{}", e))?;

                        transfers_submitted += 1;
                    }
                }
                SyncOpKind::Delete | SyncOpKind::Rename { .. } | SyncOpKind::Noop => {}
            }
        }

        // Update sync job counts in DB and RAM
        let status = if conflicts_detected > 0 && job.strategy == SyncStrategy::Manual {
            SyncStatus::Conflict
        } else {
            SyncStatus::Executing
        };

        sqlx::query(
            "UPDATE sync_jobs SET status = ?, total_files = ?, conflict_files = ?, updated_at = ? WHERE id = ?",
        )
        .bind(status.as_str())
        .bind(transfers_submitted as i64)
        .bind(conflicts_detected as i64)
        .bind(Utc::now().to_rfc3339())
        .bind(job_id)
        .execute(&self.db)
        .await?;

        Ok(transfers_submitted)
    }

    /// List all sync jobs for a user.
    pub async fn list_jobs(&self, user_id: &str) -> anyhow::Result<Vec<SyncJob>> {
        let rows = sqlx::query(
            "SELECT id, user_id, source_connection_id, source_path, destination_connection_id, destination_path, status, strategy, total_files, synced_files, conflict_files, created_at, updated_at
             FROM sync_jobs WHERE user_id = ? ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(&self.db)
        .await?;

        let mut res = Vec::with_capacity(rows.len());
        for r in rows {
            let status_str: String = r.get("status");
            let strategy_str: String = r.get("strategy");
            let created_at_str: String = r.get("created_at");
            let updated_at_str: String = r.get("updated_at");

            let status = match status_str.as_str() {
                "scanning" => SyncStatus::Scanning,
                "planning" => SyncStatus::Planning,
                "reconciling" => SyncStatus::Reconciling,
                "executing" => SyncStatus::Executing,
                "verifying" => SyncStatus::Verifying,
                "completed" => SyncStatus::Completed,
                "paused" => SyncStatus::Paused,
                "failed" => SyncStatus::Failed,
                "conflict" => SyncStatus::Conflict,
                _ => SyncStatus::Created,
            };

            use std::str::FromStr;
            let strategy = SyncStrategy::from_str(&strategy_str).unwrap_or_default();
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            res.push(SyncJob {
                id: r.get("id"),
                user_id: r.get("user_id"),
                source_connection_id: r.get("source_connection_id"),
                source_path: r.get("source_path"),
                destination_connection_id: r.get("destination_connection_id"),
                destination_path: r.get("destination_path"),
                status,
                strategy,
                total_files: r.get::<i64, _>("total_files") as u64,
                synced_files: r.get::<i64, _>("synced_files") as u64,
                conflict_files: r.get::<i64, _>("conflict_files") as u64,
                created_at,
                updated_at,
            });
        }

        Ok(res)
    }
}
