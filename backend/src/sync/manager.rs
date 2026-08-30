use crate::db::DbPool;
use crate::events::EventJournal;
use crate::runtime::TaskSupervisor;
use crate::sync::diff::ManifestDiffer;
use crate::sync::models::{SyncJob, SyncOpKind, SyncOperation, SyncStatus, SyncStrategy};
use crate::sync::scanner::VfsScanner;
use crate::transfer::TransferManager;
use crate::vfs::FileSystem;
use chrono::Utc;
use serde::Serialize;
use sqlx::Row;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
use crate::domain::VfsPath;

#[derive(Debug, Clone, Serialize)]
pub struct SyncOperationRow {
    pub id: String,
    pub job_id: String,
    pub op_kind: String,
    pub relative_path: String,
    pub old_path: Option<String>,
    pub status: String,
    pub transfer_job_id: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone)]
pub struct SyncManager {
    db: DbPool,
    transfer_manager: TransferManager,
    supervisor: TaskSupervisor,
    event_journal: Arc<EventJournal>,
    providers: Arc<RwLock<HashMap<String, Arc<dyn FileSystem>>>>,
    jobs: Arc<RwLock<HashMap<String, SyncJob>>>,
}

impl SyncManager {
    pub fn new(
        db: DbPool,
        transfer_manager: TransferManager,
        supervisor: TaskSupervisor,
        event_journal: Arc<EventJournal>,
        providers: Arc<RwLock<HashMap<String, Arc<dyn FileSystem>>>>,
    ) -> Self {
        Self {
            db,
            transfer_manager,
            supervisor,
            event_journal,
            providers,
            jobs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn supervisor(&self) -> &TaskSupervisor {
        &self.supervisor
    }

    pub async fn create_job(
        &self,
        user_id: &str,
        source_connection_id: &str,
        source_path: &str,
        destination_connection_id: &str,
        destination_path: &str,
        strategy: SyncStrategy,
    ) -> anyhow::Result<SyncJob> {
        let job = SyncJob {
            id: Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            source_connection_id: source_connection_id.to_string(),
            source_path: source_path.to_string(),
            destination_connection_id: destination_connection_id.to_string(),
            destination_path: destination_path.to_string(),
            status: SyncStatus::Created,
            strategy,
            total_files: 0,
            synced_files: 0,
            conflict_files: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        sqlx::query(
            r#"
            INSERT INTO sync_jobs (
                id, user_id, source_connection_id, source_path,
                destination_connection_id, destination_path, status, strategy,
                total_files, synced_files, conflict_files, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&job.id)
        .bind(&job.user_id)
        .bind(&job.source_connection_id)
        .bind(&job.source_path)
        .bind(&job.destination_connection_id)
        .bind(&job.destination_path)
        .bind(job.status.as_str())
        .bind(job.strategy.as_str())
        .bind(job.total_files as i64)
        .bind(job.synced_files as i64)
        .bind(job.conflict_files as i64)
        .bind(job.created_at.to_rfc3339())
        .bind(job.updated_at.to_rfc3339())
        .execute(&self.db)
        .await?;

        {
            let mut map = self.jobs.write().await;
            map.insert(job.id.clone(), job.clone());
        }

        self.start_sync_background(job.id.clone()).await;
        Ok(job)
    }

    async fn start_sync_background(&self, job_id: String) {
        let manager = self.clone();
        
        self.supervisor.spawn("sync_pipeline", async move {
            let cancel_token = CancellationToken::new(); // TODO: manage cancel tokens per job
            
            if let Err(_e) = manager.run_sync_pipeline(&job_id, cancel_token).await {
                let _ = manager.update_job_status(&job_id, SyncStatus::Failed).await;
                // Log or handle error e
            }
        });
    }

    async fn run_sync_pipeline(&self, job_id: &str, cancel: CancellationToken) -> anyhow::Result<()> {
        let job = {
            let map = self.jobs.read().await;
            map.get(job_id).cloned().ok_or_else(|| anyhow::anyhow!("Job not found"))?
        };

        self.update_job_status(job_id, SyncStatus::Scanning).await?;

        let src_fs = self.get_provider(&job.source_connection_id).await?;
        let dst_fs = self.get_provider(&job.destination_connection_id).await?;

        let source_manifests = VfsScanner::scan_directory(&src_fs, &job.source_connection_id, &job.source_path, &cancel).await?;
        let dest_manifests = VfsScanner::scan_directory(&dst_fs, &job.destination_connection_id, &job.destination_path, &cancel).await?;

        if cancel.is_cancelled() { return Ok(()); }
        self.update_job_status(job_id, SyncStatus::Planning).await?;

        let ops = ManifestDiffer::diff(&source_manifests, &dest_manifests, job.strategy);

        self.update_job_status(job_id, SyncStatus::Reconciling).await?;

        let total_files = ops.len() as u64;
        self.update_sync_job_counts(job_id, total_files, 0, 0, SyncStatus::Executing).await?;

        for op in ops {
            if cancel.is_cancelled() { return Ok(()); }
            
            let op_id = self.persist_operation(job_id, &op).await?;

            match &op.kind {
                SyncOpKind::Create | SyncOpKind::Update => {
                    let tid_res = self.transfer_manager.submit_job(
                        Some(job.user_id.clone()),
                        op.relative_path.clone(),
                        crate::transfer::engine::TransferType::Copy,
                        job.source_connection_id.clone(),
                        format!("{}/{}", job.source_path, op.relative_path),
                        job.destination_connection_id.clone(),
                        format!("{}/{}", job.destination_path, op.relative_path),
                    ).await;
                    match tid_res {
                        Ok(transfer_job_id) => {
                            self.update_operation_status(&op_id, "running", Some(&transfer_job_id), None).await?;
                        }
                        Err(e) => {
                            self.update_operation_status(&op_id, "failed", None, Some(&e.to_string())).await?;
                        }
                    }
                }
                SyncOpKind::Delete => {
                    let target = VfsPath::new(&job.destination_connection_id, format!("{}/{}", job.destination_path, op.relative_path))?;
                    match dst_fs.delete(&target).await {
                        Ok(_) => {
                            self.update_operation_status(&op_id, "completed", None, None).await?;
                            self.increment_synced(job_id).await?;
                        }
                        Err(e) => {
                            self.update_operation_status(&op_id, "failed", None, Some(&e.to_string())).await?;
                        }
                    }
                }
                SyncOpKind::Rename { old_path } => {
                    let from = VfsPath::new(&job.destination_connection_id, format!("{}/{}", job.destination_path, old_path))?;
                    let to = VfsPath::new(&job.destination_connection_id, format!("{}/{}", job.destination_path, op.relative_path))?;
                    match dst_fs.rename(&from, &to).await {
                        Ok(_) => {
                            self.update_operation_status(&op_id, "completed", None, None).await?;
                            self.increment_synced(job_id).await?;
                        }
                        Err(e) => {
                            self.update_operation_status(&op_id, "failed", None, Some(&e.to_string())).await?;
                        }
                    }
                }
                SyncOpKind::Noop => {
                    self.update_operation_status(&op_id, "completed", None, None).await?;
                    self.increment_synced(job_id).await?;
                }
                SyncOpKind::Conflict => {
                    self.update_operation_status(&op_id, "conflict", None, None).await?;
                    self.increment_conflict(job_id).await?;
                }
            }
        }
        
        // Wait, transfer operations might complete later.
        Ok(())
    }

    pub async fn notify_transfer_completed(&self, transfer_job_id: &str, success: bool) -> anyhow::Result<()> {
        let op = sqlx::query(
            "SELECT id, job_id FROM sync_operations WHERE transfer_job_id = ?"
        )
        .bind(transfer_job_id)
        .fetch_optional(&self.db)
        .await?;

        if let Some(row) = op {
            let op_id: String = row.get("id");
            let job_id: String = row.get("job_id");

            let (status, err) = if success {
                ("completed", None)
            } else {
                ("failed", Some("Transfer failed".to_string()))
            };

            self.update_operation_status(&op_id, status, Some(transfer_job_id), err.as_deref()).await?;
            if success {
                self.increment_synced(&job_id).await?;
            }

            self.check_job_completion(&job_id).await?;
        }
        Ok(())
    }
    
    async fn increment_synced(&self, job_id: &str) -> anyhow::Result<()> {
        sqlx::query("UPDATE sync_jobs SET synced_files = synced_files + 1, updated_at = ? WHERE id = ?")
            .bind(Utc::now().to_rfc3339())
            .bind(job_id)
            .execute(&self.db).await?;
        self.refresh_job(job_id).await?;
        self.check_job_completion(job_id).await?;
        Ok(())
    }

    async fn increment_conflict(&self, job_id: &str) -> anyhow::Result<()> {
        sqlx::query("UPDATE sync_jobs SET conflict_files = conflict_files + 1, updated_at = ? WHERE id = ?")
            .bind(Utc::now().to_rfc3339())
            .bind(job_id)
            .execute(&self.db).await?;
        self.refresh_job(job_id).await?;
        self.check_job_completion(job_id).await?;
        Ok(())
    }

    async fn check_job_completion(&self, job_id: &str) -> anyhow::Result<()> {
        let job = {
            let map = self.jobs.read().await;
            map.get(job_id).cloned()
        };
        if let Some(j) = job {
            if j.synced_files + j.conflict_files >= j.total_files && j.total_files > 0 {
                let status = if j.conflict_files > 0 { SyncStatus::Conflict } else { SyncStatus::Completed };
                self.update_job_status(job_id, status).await?;
            }
        }
        Ok(())
    }

    async fn refresh_job(&self, job_id: &str) -> anyhow::Result<()> {
        let row = sqlx::query("SELECT * FROM sync_jobs WHERE id = ?").bind(job_id).fetch_optional(&self.db).await?;
        if let Some(r) = row {
            let status_str: String = r.get("status");
            let strategy_str: String = r.get("strategy");
            let j = SyncJob {
                id: r.get("id"),
                user_id: r.get("user_id"),
                source_connection_id: r.get("source_connection_id"),
                source_path: r.get("source_path"),
                destination_connection_id: r.get("destination_connection_id"),
                destination_path: r.get("destination_path"),
                status: match status_str.as_str() {
                    "created" => SyncStatus::Created,
                    "scanning" => SyncStatus::Scanning,
                    "planning" => SyncStatus::Planning,
                    "reconciling" => SyncStatus::Reconciling,
                    "executing" => SyncStatus::Executing,
                    "completed" => SyncStatus::Completed,
                    "failed" => SyncStatus::Failed,
                    "conflict" => SyncStatus::Conflict,
                    _ => SyncStatus::Created,
                },
                strategy: strategy_str.parse().unwrap_or(SyncStrategy::KeepBoth),
                total_files: r.get::<i64, _>("total_files") as u64,
                synced_files: r.get::<i64, _>("synced_files") as u64,
                conflict_files: r.get::<i64, _>("conflict_files") as u64,
                created_at: Utc::now(), // simplification
                updated_at: Utc::now(),
            };
            self.jobs.write().await.insert(job_id.to_string(), j);
        }
        Ok(())
    }

    pub async fn recover_interrupted_jobs(&self) -> anyhow::Result<()> {
        let rows = sqlx::query("SELECT id, status FROM sync_jobs WHERE status IN ('scanning', 'planning', 'reconciling', 'executing')")
            .fetch_all(&self.db).await?;
        
        for row in rows {
            let id: String = row.get("id");
            let status: String = row.get("status");
            if status == "executing" {
                // Find pending transfers and resubmit (simplified: restart whole process if no transfer ID recorded)
                let pending = sqlx::query("SELECT id FROM sync_operations WHERE job_id = ? AND status IN ('pending', 'running') AND transfer_job_id IS NULL")
                    .bind(&id)
                    .fetch_all(&self.db).await?;
                if !pending.is_empty() {
                    self.start_sync_background(id).await;
                }
            } else {
                self.start_sync_background(id).await;
            }
        }
        Ok(())
    }

    pub async fn resolve_conflict(&self, job_id: &str, op_id: &str, resolution: &str) -> anyhow::Result<()> {
        // Find op
        let row = sqlx::query("SELECT relative_path FROM sync_operations WHERE id = ? AND job_id = ? AND status = 'conflict'")
            .bind(op_id).bind(job_id).fetch_optional(&self.db).await?;
        
        if let Some(r) = row {
            let rel_path: String = r.get("relative_path");
            let job = {
                let map = self.jobs.read().await;
                map.get(job_id).cloned().ok_or_else(|| anyhow::anyhow!("Job not found"))?
            };
            
            match resolution {
                "use_source" => {
                    let tid = self.transfer_manager.submit_job(
                        Some(job.user_id.clone()),
                        rel_path.clone(),
                        crate::transfer::engine::TransferType::Copy,
                        job.source_connection_id.clone(),
                        format!("{}/{}", job.source_path, rel_path),
                        job.destination_connection_id.clone(),
                        format!("{}/{}", job.destination_path, rel_path),
                    ).await.map_err(|e| anyhow::anyhow!(e))?;
                    self.update_operation_status(op_id, "running", Some(&tid), None).await?;
                }
                "use_dest" => {
                    self.update_operation_status(op_id, "completed", None, None).await?;
                    self.increment_synced(job_id).await?;
                }
                "keep_both" => {
                    // Similar to source but new name. Simplified for now
                    self.update_operation_status(op_id, "completed", None, None).await?;
                    self.increment_synced(job_id).await?;
                }
                _ => return Err(anyhow::anyhow!("Unknown resolution")),
            }
        } else {
            return Err(anyhow::anyhow!("Conflict op not found"));
        }
        Ok(())
    }

    pub async fn list_operations(&self, job_id: &str) -> anyhow::Result<Vec<SyncOperationRow>> {
        let rows = sqlx::query(
            "SELECT id, job_id, op_kind, relative_path, old_path, status, transfer_job_id, error_message, created_at, updated_at FROM sync_operations WHERE job_id = ?"
        ).bind(job_id).fetch_all(&self.db).await?;

        let mut res = Vec::new();
        for r in rows {
            res.push(SyncOperationRow {
                id: r.get("id"),
                job_id: r.get("job_id"),
                op_kind: r.get("op_kind"),
                relative_path: r.get("relative_path"),
                old_path: r.get("old_path"),
                status: r.get("status"),
                transfer_job_id: r.get("transfer_job_id"),
                error_message: r.get("error_message"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
            });
        }
        Ok(res)
    }

    pub async fn list_jobs(&self) -> anyhow::Result<Vec<SyncJob>> {
        let map = self.jobs.read().await;
        Ok(map.values().cloned().collect())
    }

    async fn get_provider(&self, conn_id: &str) -> anyhow::Result<Arc<dyn FileSystem>> {
        let p = self.providers.read().await;
        p.get(conn_id).cloned().ok_or_else(|| anyhow::anyhow!("Provider not found for {}", conn_id))
    }

    async fn update_job_status(&self, job_id: &str, status: SyncStatus) -> anyhow::Result<()> {
        sqlx::query("UPDATE sync_jobs SET status = ?, updated_at = ? WHERE id = ?")
            .bind(status.as_str())
            .bind(Utc::now().to_rfc3339())
            .bind(job_id)
            .execute(&self.db).await?;
        {
            let mut map = self.jobs.write().await;
            if let Some(j) = map.get_mut(job_id) {
                j.status = status;
                j.updated_at = Utc::now();
            }
        }
        Ok(())
    }

    async fn update_sync_job_counts(&self, job_id: &str, total: u64, synced: u64, conflicts: u64, status: SyncStatus) -> anyhow::Result<()> {
        sqlx::query("UPDATE sync_jobs SET total_files = ?, synced_files = ?, conflict_files = ?, status = ?, updated_at = ? WHERE id = ?")
            .bind(total as i64)
            .bind(synced as i64)
            .bind(conflicts as i64)
            .bind(status.as_str())
            .bind(Utc::now().to_rfc3339())
            .bind(job_id)
            .execute(&self.db).await?;
        self.refresh_job(job_id).await?;
        Ok(())
    }

    async fn persist_operation(&self, job_id: &str, op: &SyncOperation) -> anyhow::Result<String> {
        let id = Uuid::new_v4().to_string();
        let kind_str = match &op.kind {
            SyncOpKind::Create => "create",
            SyncOpKind::Update => "update",
            SyncOpKind::Delete => "delete",
            SyncOpKind::Rename { .. } => "rename",
            SyncOpKind::Noop => "noop",
            SyncOpKind::Conflict => "conflict",
        };
        let old_path = if let SyncOpKind::Rename { old_path } = &op.kind {
            Some(old_path.clone())
        } else {
            None
        };
        
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO sync_operations (id, job_id, op_kind, relative_path, old_path, status, transfer_job_id, error_message, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(job_id)
        .bind(kind_str)
        .bind(&op.relative_path)
        .bind(old_path)
        .bind("pending")
        .bind(None::<String>)
        .bind(None::<String>)
        .bind(&now)
        .bind(&now)
        .execute(&self.db).await?;
        Ok(id)
    }

    async fn update_operation_status(&self, op_id: &str, status: &str, transfer_job_id: Option<&str>, error_message: Option<&str>) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE sync_operations SET status = ?, transfer_job_id = ?, error_message = ?, updated_at = ? WHERE id = ?"
        )
        .bind(status)
        .bind(transfer_job_id)
        .bind(error_message)
        .bind(Utc::now().to_rfc3339())
        .bind(op_id)
        .execute(&self.db).await?;
        Ok(())
    }
}
