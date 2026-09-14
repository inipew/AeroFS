use crate::db::DbPool;
use crate::domain::VfsPath;
use crate::events::EventJournal;
use crate::runtime::{ResourceBudget, ResourceClass, TaskSupervisor};
use crate::sync::models::{SyncJob, SyncOpKind, SyncOperation, SyncStatus, SyncStrategy};
use crate::sync::streaming::StreamingSyncPlan;
use crate::transfer::TransferManager;
use crate::vfs::FileSystem;
use chrono::Utc;
use serde::Serialize;
use sqlx::{QueryBuilder, Row, Sqlite};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const SYNC_INSERT_CHUNK_ROWS: usize = 80;

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

struct ImmediateOutcome {
    op_id: String,
    status: &'static str,
    error_message: Option<String>,
    synced_delta: u64,
    conflict_delta: u64,
}

#[derive(Clone)]
pub struct SyncManager {
    db: DbPool,
    transfer_manager: TransferManager,
    supervisor: TaskSupervisor,
    resource_budget: Arc<ResourceBudget>,
    event_journal: Arc<EventJournal>,
    providers: Arc<RwLock<HashMap<String, Arc<dyn FileSystem>>>>,
    jobs: Arc<RwLock<HashMap<String, SyncJob>>>,
}

impl SyncManager {
    pub fn new(
        db: DbPool,
        transfer_manager: TransferManager,
        supervisor: TaskSupervisor,
        resource_budget: Arc<ResourceBudget>,
        event_journal: Arc<EventJournal>,
        providers: Arc<RwLock<HashMap<String, Arc<dyn FileSystem>>>>,
    ) -> Self {
        Self {
            db,
            transfer_manager,
            supervisor,
            resource_budget,
            event_journal,
            providers,
            jobs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn supervisor(&self) -> &TaskSupervisor {
        &self.supervisor
    }

    fn resource_class_for_connections(source: &str, destination: &str) -> ResourceClass {
        match (source == "local", destination == "local") {
            (true, true) => ResourceClass::LocalIo,
            (false, false) => ResourceClass::NetworkIo,
            _ => ResourceClass::MixedIo,
        }
    }

    async fn acquire_sync_budget(
        &self,
        job: &SyncJob,
    ) -> anyhow::Result<crate::runtime::ResourcePermit> {
        self.resource_budget
            .acquire(Self::resource_class_for_connections(
                &job.source_connection_id,
                &job.destination_connection_id,
            ))
            .await
            .map_err(|error| anyhow::anyhow!("Failed to acquire sync resource budget: {error}"))
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
            }
        });
    }

    async fn run_sync_pipeline(
        &self,
        job_id: &str,
        cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        let job = {
            let map = self.jobs.read().await;
            map.get(job_id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Job not found"))?
        };

        let _sync_permit = self.acquire_sync_budget(&job).await?;

        self.update_job_status(job_id, SyncStatus::Scanning).await?;

        let src_fs = self.get_provider(&job.source_connection_id).await?;
        let dst_fs = self.get_provider(&job.destination_connection_id).await?;

        let mut plan = StreamingSyncPlan::prepare(
            &self.db,
            Arc::clone(&src_fs),
            &job.source_connection_id,
            &job.source_path,
            Arc::clone(&dst_fs),
            &job.destination_connection_id,
            &job.destination_path,
            job.strategy,
            &cancel,
        )
        .await?;

        if cancel.is_cancelled() {
            return Ok(());
        }

        self.update_job_status(job_id, SyncStatus::Planning).await?;
        self.update_job_status(job_id, SyncStatus::Reconciling).await?;

        let total_operations = plan.total_operations();
        self.update_sync_job_counts(job_id, total_operations, 0, 0, SyncStatus::Executing)
            .await?;

        if total_operations == 0 {
            self.update_job_status(job_id, SyncStatus::Completed).await?;
            return Ok(());
        }

        while let Some(batch) = plan.next_batch().await? {
            if cancel.is_cancelled() {
                return Ok(());
            }
            self.execute_operation_batch(job_id, &job, &dst_fs, batch)
                .await?;
        }

        self.check_job_completion(job_id).await?;
        Ok(())
    }

    async fn execute_operation_batch(
        &self,
        job_id: &str,
        job: &SyncJob,
        dst_fs: &Arc<dyn FileSystem>,
        batch: Vec<SyncOperation>,
    ) -> anyhow::Result<()> {
        let persisted = self.persist_operations_batch(job_id, batch).await?;
        let mut immediate = Vec::with_capacity(persisted.len());

        for (op_id, op) in persisted {
            match &op.kind {
                SyncOpKind::Create | SyncOpKind::Update => {
                    let tid_res = self
                        .transfer_manager
                        .submit_job(
                            Some(job.user_id.clone()),
                            op.relative_path.clone(),
                            crate::transfer::engine::TransferType::Copy,
                            job.source_connection_id.clone(),
                            format!("{}/{}", job.source_path, op.relative_path),
                            job.destination_connection_id.clone(),
                            format!("{}/{}", job.destination_path, op.relative_path),
                        )
                        .await;
                    match tid_res {
                        Ok(transfer_job_id) => {
                            // Store the transfer mapping immediately. Delaying this until the end
                            // of a batch could lose a very fast transfer-completion projection.
                            self.update_operation_status_if_pending(
                                &op_id,
                                "running",
                                Some(&transfer_job_id),
                                None,
                            )
                            .await?;
                        }
                        Err(error) => {
                            self.update_operation_status_if_pending(
                                &op_id,
                                "failed",
                                None,
                                Some(&error.to_string()),
                            )
                            .await?;
                        }
                    }
                }
                SyncOpKind::Delete => {
                    let target = VfsPath::new(
                        &job.destination_connection_id,
                        format!("{}/{}", job.destination_path, op.relative_path),
                    )?;
                    match dst_fs.delete(&target).await {
                        Ok(_) => immediate.push(ImmediateOutcome {
                            op_id,
                            status: "completed",
                            error_message: None,
                            synced_delta: 1,
                            conflict_delta: 0,
                        }),
                        Err(error) => immediate.push(ImmediateOutcome {
                            op_id,
                            status: "failed",
                            error_message: Some(error.to_string()),
                            synced_delta: 0,
                            conflict_delta: 0,
                        }),
                    }
                }
                SyncOpKind::Rename { old_path } => {
                    let from = VfsPath::new(
                        &job.destination_connection_id,
                        format!("{}/{}", job.destination_path, old_path),
                    )?;
                    let to = VfsPath::new(
                        &job.destination_connection_id,
                        format!("{}/{}", job.destination_path, op.relative_path),
                    )?;
                    match dst_fs.rename(&from, &to).await {
                        Ok(_) => immediate.push(ImmediateOutcome {
                            op_id,
                            status: "completed",
                            error_message: None,
                            synced_delta: 1,
                            conflict_delta: 0,
                        }),
                        Err(error) => immediate.push(ImmediateOutcome {
                            op_id,
                            status: "failed",
                            error_message: Some(error.to_string()),
                            synced_delta: 0,
                            conflict_delta: 0,
                        }),
                    }
                }
                SyncOpKind::Noop => immediate.push(ImmediateOutcome {
                    op_id,
                    status: "completed",
                    error_message: None,
                    synced_delta: 1,
                    conflict_delta: 0,
                }),
                SyncOpKind::Conflict => immediate.push(ImmediateOutcome {
                    op_id,
                    status: "conflict",
                    error_message: None,
                    synced_delta: 0,
                    conflict_delta: 1,
                }),
            }
        }

        self.apply_immediate_outcomes(job_id, immediate).await
    }

    /// Applies a transfer lifecycle result to its sync operation exactly once.
    pub async fn notify_transfer_completed(
        &self,
        transfer_job_id: &str,
        success: bool,
    ) -> anyhow::Result<()> {
        let mut tx = self.db.begin().await?;
        let op =
            sqlx::query("SELECT id, job_id, status FROM sync_operations WHERE transfer_job_id = ?")
                .bind(transfer_job_id)
                .fetch_optional(&mut *tx)
                .await?;

        let Some(row) = op else {
            tx.commit().await?;
            return Ok(());
        };

        let op_id: String = row.get("id");
        let job_id: String = row.get("job_id");
        let current_status: String = row.get("status");

        if matches!(current_status.as_str(), "completed" | "failed") {
            tx.commit().await?;
            return Ok(());
        }

        let (status, error_message) = if success {
            ("completed", None)
        } else {
            ("failed", Some("Transfer failed"))
        };
        let now = Utc::now().to_rfc3339();

        let updated = sqlx::query(
            "UPDATE sync_operations\n             SET status = ?, transfer_job_id = ?, error_message = ?, updated_at = ?\n             WHERE id = ? AND status NOT IN ('completed', 'failed')",
        )
        .bind(status)
        .bind(transfer_job_id)
        .bind(error_message)
        .bind(&now)
        .bind(&op_id)
        .execute(&mut *tx)
        .await?;

        if updated.rows_affected() == 1 && success {
            sqlx::query(
                "UPDATE sync_jobs SET synced_files = synced_files + 1, updated_at = ? WHERE id = ?",
            )
            .bind(&now)
            .bind(&job_id)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        if updated.rows_affected() == 1 {
            self.refresh_job(&job_id).await?;
            self.check_job_completion(&job_id).await?;
        }
        Ok(())
    }

    async fn increment_synced(&self, job_id: &str) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE sync_jobs SET synced_files = synced_files + 1, updated_at = ? WHERE id = ?",
        )
        .bind(Utc::now().to_rfc3339())
        .bind(job_id)
        .execute(&self.db)
        .await?;
        self.refresh_job(job_id).await?;
        self.check_job_completion(job_id).await?;
        Ok(())
    }

    async fn increment_conflict(&self, job_id: &str) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE sync_jobs SET conflict_files = conflict_files + 1, updated_at = ? WHERE id = ?",
        )
        .bind(Utc::now().to_rfc3339())
        .bind(job_id)
        .execute(&self.db)
        .await?;
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
            if j.synced_files + j.conflict_files >= j.total_files {
                let status = if j.conflict_files > 0 {
                    SyncStatus::Conflict
                } else {
                    SyncStatus::Completed
                };
                self.update_job_status(job_id, status).await?;
            }
        }
        Ok(())
    }

    async fn refresh_job(&self, job_id: &str) -> anyhow::Result<()> {
        let row = sqlx::query("SELECT * FROM sync_jobs WHERE id = ?")
            .bind(job_id)
            .fetch_optional(&self.db)
            .await?;
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
                created_at: Utc::now(),
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
            self.refresh_job(&id).await?;

            if status == "executing" {
                let job_opt = {
                    let map = self.jobs.read().await;
                    map.get(&id).cloned()
                };
                let job = match job_opt {
                    Some(j) => j,
                    None => continue,
                };
                let _sync_permit = self.acquire_sync_budget(&job).await?;

                let ops = self.list_operations(&id).await?;
                let mut all_completed = true;

                for op in ops {
                    if op.status == "completed" {
                        continue;
                    }
                    if op.status == "conflict" {
                        all_completed = false;
                        continue;
                    }

                    all_completed = false;
                    if let Some(ref tid) = op.transfer_job_id {
                        let transfer_jobs = self.transfer_manager.list_jobs(None, true, true).await;
                        if let Some(tj) = transfer_jobs.iter().find(|t| &t.id == tid) {
                            match tj.status {
                                crate::transfer::engine::TransferStatus::Completed => {
                                    self.notify_transfer_completed(tid, true).await?;
                                }
                                crate::transfer::engine::TransferStatus::Failed
                                | crate::transfer::engine::TransferStatus::Interrupted => {
                                    let _ = self.transfer_manager.retry_job(tid, None, true).await;
                                }
                                _ => {}
                            }
                            continue;
                        }
                    }

                    let dst_fs = match self.get_provider(&job.destination_connection_id).await {
                        Ok(fs) => fs,
                        Err(_) => continue,
                    };

                    match op.op_kind.as_str() {
                        "create" | "update" => {
                            let tid_res = self
                                .transfer_manager
                                .submit_job(
                                    Some(job.user_id.clone()),
                                    op.relative_path.clone(),
                                    crate::transfer::engine::TransferType::Copy,
                                    job.source_connection_id.clone(),
                                    format!("{}/{}", job.source_path, op.relative_path),
                                    job.destination_connection_id.clone(),
                                    format!("{}/{}", job.destination_path, op.relative_path),
                                )
                                .await;
                            if let Ok(tid) = tid_res {
                                self.update_operation_status(&op.id, "running", Some(&tid), None)
                                    .await?;
                            }
                        }
                        "delete" => {
                            if let Ok(target) = VfsPath::new(
                                &job.destination_connection_id,
                                format!("{}/{}", job.destination_path, op.relative_path),
                            ) {
                                if dst_fs.delete(&target).await.is_ok() {
                                    self.update_operation_status(&op.id, "completed", None, None)
                                        .await?;
                                    self.increment_synced(&id).await?;
                                }
                            }
                        }
                        "rename" => {
                            if let Some(old) = op.old_path {
                                if let (Ok(from), Ok(to)) = (
                                    VfsPath::new(
                                        &job.destination_connection_id,
                                        format!("{}/{}", job.destination_path, old),
                                    ),
                                    VfsPath::new(
                                        &job.destination_connection_id,
                                        format!("{}/{}", job.destination_path, op.relative_path),
                                    ),
                                ) {
                                    if dst_fs.rename(&from, &to).await.is_ok() {
                                        self.update_operation_status(
                                            &op.id,
                                            "completed",
                                            None,
                                            None,
                                        )
                                        .await?;
                                        self.increment_synced(&id).await?;
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }

                if all_completed {
                    self.check_job_completion(&id).await?;
                }
            } else {
                self.start_sync_background(id).await;
            }
        }
        Ok(())
    }

    pub async fn resolve_conflict(
        &self,
        job_id: &str,
        op_id: &str,
        resolution: &str,
    ) -> anyhow::Result<()> {
        let row = sqlx::query("SELECT relative_path FROM sync_operations WHERE id = ? AND job_id = ? AND status = 'conflict'")
            .bind(op_id).bind(job_id).fetch_optional(&self.db).await?;

        if let Some(r) = row {
            let rel_path: String = r.get("relative_path");
            let job = {
                let map = self.jobs.read().await;
                map.get(job_id)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("Job not found"))?
            };

            match resolution {
                "use_source" => {
                    let tid = self
                        .transfer_manager
                        .submit_job(
                            Some(job.user_id.clone()),
                            rel_path.clone(),
                            crate::transfer::engine::TransferType::Copy,
                            job.source_connection_id.clone(),
                            format!("{}/{}", job.source_path, rel_path),
                            job.destination_connection_id.clone(),
                            format!("{}/{}", job.destination_path, rel_path),
                        )
                        .await
                        .map_err(|e| anyhow::anyhow!(e))?;
                    self.update_operation_status(op_id, "running", Some(&tid), None)
                        .await?;
                }
                "use_dest" => {
                    self.update_operation_status(op_id, "completed", None, None)
                        .await?;
                    self.increment_synced(job_id).await?;
                }
                "keep_both" => {
                    let now_tag = Utc::now().format("%Y%m%d_%H%M%S").to_string();
                    let new_rel_path = if let Some(dot_idx) = rel_path.rfind('.') {
                        format!(
                            "{}.sync-conflict-{}{}",
                            &rel_path[..dot_idx],
                            now_tag,
                            &rel_path[dot_idx..]
                        )
                    } else {
                        format!("{}.sync-conflict-{}", rel_path, now_tag)
                    };

                    let tid = self
                        .transfer_manager
                        .submit_job(
                            Some(job.user_id.clone()),
                            new_rel_path.clone(),
                            crate::transfer::engine::TransferType::Copy,
                            job.source_connection_id.clone(),
                            format!("{}/{}", job.source_path, rel_path),
                            job.destination_connection_id.clone(),
                            format!("{}/{}", job.destination_path, new_rel_path),
                        )
                        .await
                        .map_err(|e| anyhow::anyhow!(e))?;

                    self.update_operation_status(op_id, "running", Some(&tid), None)
                        .await?;
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
        p.get(conn_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Provider not found for {}", conn_id))
    }

    async fn update_job_status(&self, job_id: &str, status: SyncStatus) -> anyhow::Result<()> {
        sqlx::query("UPDATE sync_jobs SET status = ?, updated_at = ? WHERE id = ?")
            .bind(status.as_str())
            .bind(Utc::now().to_rfc3339())
            .bind(job_id)
            .execute(&self.db)
            .await?;
        {
            let mut map = self.jobs.write().await;
            if let Some(j) = map.get_mut(job_id) {
                j.status = status;
                j.updated_at = Utc::now();
            }
        }
        Ok(())
    }

    async fn update_sync_job_counts(
        &self,
        job_id: &str,
        total: u64,
        synced: u64,
        conflicts: u64,
        status: SyncStatus,
    ) -> anyhow::Result<()> {
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

    fn operation_kind(op: &SyncOperation) -> (&'static str, Option<&str>) {
        match &op.kind {
            SyncOpKind::Create => ("create", None),
            SyncOpKind::Update => ("update", None),
            SyncOpKind::Delete => ("delete", None),
            SyncOpKind::Rename { old_path } => ("rename", Some(old_path.as_str())),
            SyncOpKind::Noop => ("noop", None),
            SyncOpKind::Conflict => ("conflict", None),
        }
    }

    async fn persist_operations_batch(
        &self,
        job_id: &str,
        operations: Vec<SyncOperation>,
    ) -> anyhow::Result<Vec<(String, SyncOperation)>> {
        if operations.is_empty() {
            return Ok(Vec::new());
        }

        let persisted: Vec<(String, SyncOperation)> = operations
            .into_iter()
            .map(|operation| (Uuid::new_v4().to_string(), operation))
            .collect();
        let now = Utc::now().to_rfc3339();
        let mut tx = self.db.begin().await?;

        // Ten bound parameters per row. Chunking keeps this below conservative SQLite bind
        // limits while still reducing 256 individual INSERT transactions to a few multi-row SQL
        // statements inside one transaction.
        for chunk in persisted.chunks(SYNC_INSERT_CHUNK_ROWS) {
            let mut builder = QueryBuilder::<Sqlite>::new(
                "INSERT INTO sync_operations (id, job_id, op_kind, relative_path, old_path, status, transfer_job_id, error_message, created_at, updated_at) ",
            );
            builder.push_values(chunk, |mut row, (id, operation)| {
                let (kind, old_path) = Self::operation_kind(operation);
                row.push_bind(id)
                    .push_bind(job_id)
                    .push_bind(kind)
                    .push_bind(&operation.relative_path)
                    .push_bind(old_path)
                    .push_bind("pending")
                    .push_bind(None::<String>)
                    .push_bind(None::<String>)
                    .push_bind(&now)
                    .push_bind(&now);
            });
            builder.build().execute(&mut *tx).await?;
        }

        tx.commit().await?;
        Ok(persisted)
    }

    async fn apply_immediate_outcomes(
        &self,
        job_id: &str,
        outcomes: Vec<ImmediateOutcome>,
    ) -> anyhow::Result<()> {
        if outcomes.is_empty() {
            return Ok(());
        }

        let now = Utc::now().to_rfc3339();
        let mut tx = self.db.begin().await?;
        let mut synced_delta = 0u64;
        let mut conflict_delta = 0u64;

        for outcome in outcomes {
            let updated = sqlx::query(
                "UPDATE sync_operations SET status = ?, error_message = ?, updated_at = ? WHERE id = ? AND status = 'pending'",
            )
            .bind(outcome.status)
            .bind(outcome.error_message.as_deref())
            .bind(&now)
            .bind(&outcome.op_id)
            .execute(&mut *tx)
            .await?;

            if updated.rows_affected() == 1 {
                synced_delta += outcome.synced_delta;
                conflict_delta += outcome.conflict_delta;
            }
        }

        if synced_delta > 0 || conflict_delta > 0 {
            sqlx::query(
                "UPDATE sync_jobs SET synced_files = synced_files + ?, conflict_files = conflict_files + ?, updated_at = ? WHERE id = ?",
            )
            .bind(synced_delta as i64)
            .bind(conflict_delta as i64)
            .bind(&now)
            .bind(job_id)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        if synced_delta > 0 || conflict_delta > 0 {
            self.refresh_job(job_id).await?;
            self.check_job_completion(job_id).await?;
        }
        Ok(())
    }

    async fn update_operation_status_if_pending(
        &self,
        op_id: &str,
        status: &str,
        transfer_job_id: Option<&str>,
        error_message: Option<&str>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE sync_operations SET status = ?, transfer_job_id = ?, error_message = ?, updated_at = ? WHERE id = ? AND status = 'pending'",
        )
        .bind(status)
        .bind(transfer_job_id)
        .bind(error_message)
        .bind(Utc::now().to_rfc3339())
        .bind(op_id)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    async fn update_operation_status(
        &self,
        op_id: &str,
        status: &str,
        transfer_job_id: Option<&str>,
        error_message: Option<&str>,
    ) -> anyhow::Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_resource_class_tracks_endpoint_pressure() {
        assert_eq!(
            SyncManager::resource_class_for_connections("local", "local"),
            ResourceClass::LocalIo
        );
        assert_eq!(
            SyncManager::resource_class_for_connections("sftp-a", "s3-b"),
            ResourceClass::NetworkIo
        );
        assert_eq!(
            SyncManager::resource_class_for_connections("local", "sftp-a"),
            ResourceClass::MixedIo
        );
    }

    #[test]
    fn operation_kind_preserves_rename_source() {
        let op = SyncOperation {
            relative_path: "new.txt".into(),
            kind: SyncOpKind::Rename {
                old_path: "old.txt".into(),
            },
            source_manifest: None,
            dest_manifest: None,
        };
        assert_eq!(SyncManager::operation_kind(&op), ("rename", Some("old.txt")));
    }
}
