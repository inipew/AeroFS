use super::planner::{TransferPlanner, TransferStrategy};
use crate::db::DbPool;
use crate::domain::VfsPath;
pub use crate::events::EventEnvelope;
pub use crate::transfer::model::{
    TransferExecutionMode, TransferJob, TransferPhase, TransferStaging, TransferStatus,
    TransferType,
};
use crate::events::{DomainEvent, EventJournal, ReplayOutcome};
use crate::vfs::FileSystem;
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub type WsEvent = DomainEvent;
pub type ReplayResult = ReplayOutcome;


#[derive(Clone)]
pub struct TransferManager {
    jobs: Arc<RwLock<HashMap<String, TransferJob>>>,
    cancel_tokens: Arc<RwLock<HashMap<String, CancellationToken>>>,
    queue_tx: mpsc::Sender<String>,
    event_journal: Arc<EventJournal>,
    db: DbPool,
    max_concurrent_workers: Arc<AtomicUsize>,
    max_retry_attempts: Arc<AtomicUsize>,
    worker_semaphore: Arc<tokio::sync::Semaphore>,
    is_accepting_jobs: Arc<std::sync::atomic::AtomicBool>,
    completion_tx: broadcast::Sender<(String, bool)>,
}

impl TransferManager {
    /// Create and initialize the TransferManager.
    /// Recovery is performed synchronously (awaited) before returning, so the server
    /// only announces readiness after all persisted jobs are loaded into memory.
    /// The scheduler and recovery tasks are registered in `task_tracker` so they respond
    /// to `shutdown_token` and are drained cleanly on shutdown.
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        providers: Arc<RwLock<HashMap<String, Arc<dyn FileSystem>>>>,
        db: DbPool,
        max_concurrent_workers: usize,
        event_journal: Arc<EventJournal>,
        shutdown_token: CancellationToken,
        task_tracker: &tokio_util::task::TaskTracker,
    ) -> Self {
        let (queue_tx, queue_rx) = mpsc::channel::<String>(200);
        let (completion_tx, _) = broadcast::channel::<(String, bool)>(400);

        let jobs: Arc<RwLock<HashMap<String, TransferJob>>> = Arc::new(RwLock::new(HashMap::new()));
        let cancel_tokens: Arc<RwLock<HashMap<String, CancellationToken>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let clamped_workers = max_concurrent_workers.clamp(1, 64);
        let max_concurrent_workers_arc = Arc::new(AtomicUsize::new(clamped_workers));
        let max_retry_attempts_arc = Arc::new(AtomicUsize::new(3));
        let worker_semaphore = Arc::new(tokio::sync::Semaphore::new(clamped_workers));
        let is_accepting_jobs = Arc::new(std::sync::atomic::AtomicBool::new(true));

        let jobs_clone = Arc::clone(&jobs);
        let cancel_tokens_clone = Arc::clone(&cancel_tokens);
        let event_journal_clone = Arc::clone(&event_journal);
        let db_clone = db.clone();
        let queue_tx_clone = queue_tx.clone();
        let completion_tx_clone = completion_tx.clone();

        // 1. Synchronous startup recovery: Load jobs from SQLite into memory
        // Awaited directly so server readiness is announced only after recovery completes.
        {
            let db_init = db.clone();
            let jobs_init = Arc::clone(&jobs);
            if let Ok(saved_jobs) = Self::load_jobs_from_db(&db_init).await {
                let mut map = jobs_init.write().await;
                for mut job in saved_jobs {
                    // Only keep non-dismissed and active/recent jobs in RAM
                    if job.dismissed_at.is_some() {
                        continue;
                    }
                    match job.status {
                        TransferStatus::Running => {
                            job.status = TransferStatus::Interrupted;
                            job.error_message =
                                Some("Transfer interrupted by server restart".into());
                            tracing::info!("transfer.interrupted: job_id={}", job.id);
                            let _ = Self::save_job_to_db(&db_init, &job).await;
                        }
                        TransferStatus::CancellationRequested => {
                            job.status = TransferStatus::Cancelled;
                            job.speed_bytes_per_sec = 0;
                            job.eta_seconds = None;
                            job.updated_at = Utc::now();
                            tracing::info!("transfer.cancelled_on_restart: job_id={}", job.id);
                            let _ = Self::save_job_to_db(&db_init, &job).await;
                        }
                        TransferStatus::Queued => {
                            let _ = queue_tx_clone.send(job.id.clone()).await;
                        }
                        _ => {}
                    }
                    map.insert(job.id.clone(), job);
                }
            }
            tracing::info!("transfer.recovery: completed");
        }

        // 2. Multi-Worker Concurrent Transfer Scheduler — registered in task_tracker
        // so it is drained properly on shutdown. Scheduler stops accepting new work when
        // shutdown_token fires.
        let queue_rx_shared = Arc::new(Mutex::new(queue_rx));
        let retries_clone = Arc::clone(&max_retry_attempts_arc);
        let worker_semaphore_task = Arc::clone(&worker_semaphore);
        let is_accepting_clone = Arc::clone(&is_accepting_jobs);
        let scheduler_token = shutdown_token.clone();
        let tracker_clone = task_tracker.clone();

        task_tracker.spawn(async move {
            tracing::debug!("transfer.scheduler.start");
            let mut rx = queue_rx_shared.lock().await;
            loop {
                // Wait for next job first (with shutdown guard), then acquire concurrency permit
                let job_id = tokio::select! {
                    _ = scheduler_token.cancelled() => {
                        tracing::info!("transfer.scheduler.stop: shutdown requested");
                        is_accepting_clone.store(false, std::sync::atomic::Ordering::Release);
                        break;
                    }
                    maybe_id = rx.recv() => {
                        match maybe_id {
                            Some(id) => id,
                            None => break, // channel closed
                        }
                    }
                };

                let permit = tokio::select! {
                    _ = scheduler_token.cancelled() => {
                        tracing::info!("transfer.scheduler.stop: shutdown requested (permit wait)");
                        is_accepting_clone.store(false, std::sync::atomic::Ordering::Release);
                        break;
                    }
                    result = worker_semaphore_task.clone().acquire_owned() => {
                        match result {
                            Ok(p) => p,
                            Err(_) => break, // semaphore closed
                        }
                    }
                };

                let jobs_worker = Arc::clone(&jobs_clone);
                let cancel_tokens_worker = Arc::clone(&cancel_tokens_clone);
                let providers_worker = Arc::clone(&providers);
                let event_journal_worker = Arc::clone(&event_journal_clone);
                let db_worker = db_clone.clone();
                let completion_tx_worker = completion_tx_clone.clone();
                let retries_task = Arc::clone(&retries_clone);
                let tracker_for_worker = tracker_clone.clone();

                tracker_for_worker.spawn(async move {
                    let _permit = permit;
                    let cancel_token = {
                        let mut tokens = cancel_tokens_worker.write().await;
                        tokens
                            .entry(job_id.clone())
                            .or_insert_with(CancellationToken::new)
                            .clone()
                    };

                    let (should_run, job_opt) = {
                        let mut map = jobs_worker.write().await;
                        if let Some(j) = map.get_mut(&job_id) {
                            if j.status == TransferStatus::Cancelled {
                                (false, Some(j.clone()))
                            } else if j.status == TransferStatus::CancellationRequested
                                || cancel_token.is_cancelled()
                            {
                                j.status = TransferStatus::Cancelled;
                                j.speed_bytes_per_sec = 0;
                                j.eta_seconds = None;
                                j.updated_at = Utc::now();
                                (false, Some(j.clone()))
                            } else {
                                j.status = TransferStatus::Running;
                                j.updated_at = Utc::now();
                                (true, Some(j.clone()))
                            }
                        } else {
                            (false, None)
                        }
                    };

                    if let Some(mut job) = job_opt {
                        if !should_run {
                            if job.status == TransferStatus::Cancelled {
                                let _ = Self::save_job_to_db(&db_worker, &job).await;
                                let _ = event_journal_worker.append(
                                    DomainEvent::transfer_failed(&job),
                                    Some(&job.id),
                                ).await;
                            }
                        } else {
                            let _ = Self::save_job_to_db(&db_worker, &job).await;
                            let _ = event_journal_worker.append(
                                DomainEvent::transfer_progress(&job),
                                Some(&job.id),
                            ).await;

                            // Execute robust bounded-stream transfer with retry & instant CancellationToken abort
                            let result = Self::execute_job_with_retry(
                                &mut job,
                                &cancel_token,
                                &providers_worker,
                                &jobs_worker,
                                &event_journal_worker,
                                &db_worker,
                                &retries_task,
                            )
                            .await;

                            // Re-read fresh status in case user requested cancellation during transfer
                            let current_status = {
                                let map = jobs_worker.read().await;
                                map.get(&job.id).map(|j| j.status).unwrap_or(job.status)
                            };

                            if current_status == TransferStatus::Cancelled
                                || current_status == TransferStatus::CancellationRequested
                                || cancel_token.is_cancelled()
                            {
                                job.status = TransferStatus::Cancelled;
                                job.speed_bytes_per_sec = 0;
                                job.eta_seconds = None;
                                job.updated_at = Utc::now();
                                {
                                    let mut map = jobs_worker.write().await;
                                    map.insert(job.id.clone(), job.clone());
                                }
                                let _ = Self::save_job_conditional(&db_worker, &job, &["cancelled", "cancellation_requested", "running", "queued"]).await;
                                let _ = event_journal_worker.append(
                                    DomainEvent::transfer_failed(&job),
                                    Some(&job.id),
                                ).await;
                            } else {
                                match result {
                                    Ok(()) => {
                                        // Double-check cancellation atomically before marking completed (race window fix)
                                        let still_cancelled = {
                                            let map = jobs_worker.read().await;
                                            map.get(&job.id).map(|j| j.status == TransferStatus::Cancelled || j.status == TransferStatus::CancellationRequested).unwrap_or(false)
                                        } || cancel_token.is_cancelled();
                                        if still_cancelled {
                                            job.status = TransferStatus::Cancelled;
                                            job.speed_bytes_per_sec = 0;
                                            job.eta_seconds = None;
                                            job.updated_at = Utc::now();
                                            {
                                                let mut map = jobs_worker.write().await;
                                                map.insert(job.id.clone(), job.clone());
                                            }
                                            let _ = Self::save_job_to_db(&db_worker, &job).await;
                                            let _ = event_journal_worker.append(DomainEvent::transfer_failed(&job), Some(&job.id)).await;
                                        } else {
                                            let _ = crate::transfer::checkpoint::TransferCheckpoint::delete(&db_worker, &job.id).await;
                                            job.status = TransferStatus::Completed;
                                            job.phase = TransferPhase::Completed;
                                            job.speed_bytes_per_sec = 0;
                                            job.eta_seconds = Some(0);
                                            job.updated_at = Utc::now();
                                            {
                                                let mut map = jobs_worker.write().await;
                                                map.insert(job.id.clone(), job.clone());
                                            }
                                            let _ = Self::save_job_conditional_completed(&db_worker, &job).await;
                                        // 1. Broadcast real-time FileChange event FIRST so open panels auto-refresh immediately (Plan 41 #22)
                                        let _ = event_journal_worker.append(
                                            DomainEvent::file_change(
                                                &job.destination_connection_id,
                                                &job.destination_path,
                                                "create",
                                            ),
                                            Some(&job.id),
                                        ).await;
                                        if job.transfer_type == TransferType::Move {
                                            let _ = event_journal_worker.append(
                                                DomainEvent::file_change(
                                                    &job.source_connection_id,
                                                    &job.source_path,
                                                    "delete",
                                                ),
                                                Some(&job.id),
                                            ).await;
                                        }

                                        // 2. Then emit TransferCompleted
                                        let _ = event_journal_worker.append(
                                            DomainEvent::transfer_completed(&job),
                                            Some(&job.id),
                                        ).await;
                                        let _ = completion_tx_worker.send((job.id.clone(), true));
                                        }
                                    }
                                    Err(e) => {
                                        job.status = TransferStatus::Failed;
                                        job.error_message = Some(e.to_string());
                                        job.speed_bytes_per_sec = 0;
                                        job.eta_seconds = None;
                                        job.updated_at = Utc::now();
                                        {
                                            let mut map = jobs_worker.write().await;
                                            map.insert(job.id.clone(), job.clone());
                                        }
                                        let _ = Self::save_job_to_db(&db_worker, &job).await;
                                        let _ = event_journal_worker.append(
                                            DomainEvent::transfer_failed(&job),
                                            Some(&job.id),
                                        ).await;
                                        let _ = completion_tx_worker.send((job.id.clone(), false));
                                    }
                                }
                            }
                        }
                    }

                    // Clean up cancellation token
                    cancel_tokens_worker.write().await.remove(&job_id);
                });
            }
        });

        Self {
            jobs,
            cancel_tokens,
            queue_tx,
            event_journal,
            db,
            max_concurrent_workers: max_concurrent_workers_arc,
            max_retry_attempts: max_retry_attempts_arc,
            worker_semaphore,
            is_accepting_jobs,
            completion_tx,
        }
    }

    pub fn completion_receiver(&self) -> broadcast::Receiver<(String, bool)> {
        self.completion_tx.subscribe()
    }

    /// Transitional accessor for manager-owned cancellation token (P0).
    /// Returns cloned token to avoid borrow coupling. Executor must not create its own token.
    pub fn cancel_token(&self, job_id: &str) -> Option<CancellationToken> {
        self.cancel_tokens.try_read().ok()?.get(job_id).cloned()
    }

    /// Dynamically update transfer concurrency worker limit and max retry count without restart (P1 #16 & #17)
    pub fn update_limits(&self, max_concurrent: usize, max_retries: usize) {
        let clamped_workers = max_concurrent.clamp(1, 64);
        let old_workers = self
            .max_concurrent_workers
            .swap(clamped_workers, Ordering::SeqCst);
        if clamped_workers > old_workers {
            self.worker_semaphore
                .add_permits(clamped_workers - old_workers);
        }
        self.max_retry_attempts
            .store(max_retries.clamp(1, 10), Ordering::SeqCst);
    }

    pub fn set_max_concurrent_transfers(&self, max_concurrent: usize) {
        self.update_limits(
            max_concurrent,
            self.max_retry_attempts.load(Ordering::Relaxed),
        );
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EventEnvelope> {
        self.event_journal.subscribe()
    }

    pub fn current_sequence(&self) -> u64 {
        self.event_journal.latest_sequence()
    }

    pub async fn get_events_since(&self, since_seq: u64) -> ReplayOutcome {
        self.event_journal
            .get_since(Some(self.event_journal.epoch()), since_seq, 500)
            .await
            .unwrap_or(ReplayOutcome::Events(Vec::new()))
    }

    pub async fn broadcast_event(&self, event: DomainEvent) {
        let _ = self.event_journal.append(event, None).await;
    }

    pub async fn emit_event(&self, event: DomainEvent, aggregate_id: Option<&str>) {
        let _ = self.event_journal.append(event, aggregate_id).await;
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn submit_job(
        &self,
        user_id: Option<String>,
        name: String,
        transfer_type: TransferType,
        source_connection_id: String,
        source_path: String,
        destination_connection_id: String,
        destination_path: String,
    ) -> Result<String, String> {
        // Reject new jobs during shutdown to prevent half-lifecycle transfers
        if !self
            .is_accepting_jobs
            .load(std::sync::atomic::Ordering::Acquire)
        {
            return Err("Server is shutting down; no new transfers accepted".to_string());
        }

        let id = format!("job_{}", &Uuid::new_v4().to_string()[..8]);
        let now = Utc::now();

        let job = TransferJob {
            id: id.clone(),
            user_id,
            name,
            transfer_type,
            source_connection_id,
            source_path,
            destination_connection_id,
            destination_path,
            status: TransferStatus::Queued,
            phase: TransferPhase::Preparing,
            execution_mode: TransferExecutionMode::Inline,
            staging: TransferStaging::None,
            transferred_bytes: 0,
            total_bytes: 0,
            speed_bytes_per_sec: 0,
            eta_seconds: None,
            checksum: None,
            error_message: None,
            dismissed_at: None,
            created_at: now,
            updated_at: now,
        };

        // 1. Save to SQLite for durability
        Self::save_job_to_db(&self.db, &job)
            .await
            .map_err(|e| format!("Database persistence error: {}", e))?;

        // 2. Insert CancellationToken first to close cancel-before-token race (RACE-3)
        {
            let mut tokens = self.cancel_tokens.write().await;
            tokens.insert(id.clone(), CancellationToken::new());
        }
        {
            let mut map = self.jobs.write().await;
            map.insert(id.clone(), job.clone());
        }

        let _ = self
            .event_journal
            .append(DomainEvent::transfer_progress(&job), Some(&job.id))
            .await;
        self.queue_tx
            .send(id.clone())
            .await
            .map_err(|e| format!("Failed to queue transfer job: {}", e))?;

        Ok(id)
    }

    /// Create an inline Upload transfer job owned by TransferEngine (Upload-as-Transfer).
    /// Does NOT queue — caller executes inline and must call `complete_inline_job` / `fail_inline_job`.
    /// Prefer `create_inline_upload_job_with_plan` (single source of truth via TransferPlan).
    pub async fn create_inline_upload_job(
        &self,
        user_id: Option<String>,
        name: String,
        dest_connection_id: String,
        dest_path: String,
        total_bytes: Option<u64>,
        staging: TransferStaging,
        execution_mode: TransferExecutionMode,
    ) -> TransferJob {
        let id = format!("job_{}", &Uuid::new_v4().to_string()[..8]);
        let now = Utc::now();
        let job = TransferJob {
            id: id.clone(),
            user_id,
            name,
            transfer_type: TransferType::Upload,
            source_connection_id: "upload".to_string(),
            source_path: format!("upload://{}", id),
            destination_connection_id: dest_connection_id,
            destination_path: dest_path,
            status: TransferStatus::Running,
            phase: TransferPhase::Transferring,
            execution_mode,
            staging,
            transferred_bytes: 0,
            total_bytes: total_bytes.unwrap_or(0),
            speed_bytes_per_sec: 0,
            eta_seconds: None,
            checksum: None,
            error_message: None,
            dismissed_at: None,
            created_at: now,
            updated_at: now,
        };
        {
            let mut tokens = self.cancel_tokens.write().await;
            tokens.insert(id.clone(), CancellationToken::new());
        }
        {
            let mut map = self.jobs.write().await;
            map.insert(id.clone(), job.clone());
        }
        let _ = self.event_journal.append(DomainEvent::transfer_progress(&job), Some(&job.id)).await;
        let _ = Self::save_job_to_db(&self.db, &job).await;
        job
    }

    /// Create inline upload job from a unified TransferPlan (P0 single source of truth).
    pub async fn create_inline_upload_job_with_plan(
        &self,
        user_id: Option<String>,
        name: String,
        dest_connection_id: String,
        dest_path: String,
        total_bytes: Option<u64>,
        plan: crate::transfer::plan::TransferPlan,
    ) -> TransferJob {
        self.create_inline_upload_job(
            user_id,
            name,
            dest_connection_id,
            dest_path,
            total_bytes,
            plan.staging,
            plan.execution_mode,
        )
        .await
    }

    /// Helper: canonical staging target for an upload job (unified naming).
    pub fn upload_staging_target(&self, target: &crate::domain::VfsPath, job_id: &str, plan: &crate::transfer::plan::TransferPlan) -> Option<crate::domain::VfsPath> {
        plan.staging_path(target, job_id)
    }

    pub async fn update_inline_progress(&self, job_id: &str, transferred: u64, total: u64, speed: u64, eta: Option<u64>) {
        let mut map = self.jobs.write().await;
        if let Some(j) = map.get_mut(job_id) {
            j.transferred_bytes = transferred;
            j.total_bytes = total;
            j.speed_bytes_per_sec = speed;
            j.eta_seconds = eta;
            j.updated_at = Utc::now();
            let job = j.clone();
            drop(map);
            let _ = self.event_journal.append(DomainEvent::transfer_progress(&job), Some(&job.id)).await;
            let _ = Self::save_job_to_db(&self.db, &job).await;
        }
    }

    pub async fn complete_inline_job(&self, job_id: &str, checksum: Option<String>) {
        let job_opt = {
            let mut map = self.jobs.write().await;
            if let Some(j) = map.get_mut(job_id) {
                j.status = TransferStatus::Completed;
                j.phase = TransferPhase::Completed;
                j.speed_bytes_per_sec = 0;
                j.eta_seconds = Some(0);
                j.checksum = checksum.clone();
                j.updated_at = Utc::now();
                Some(j.clone())
            } else { None }
        };
        if let Some(job) = job_opt {
            let _ = Self::save_job_conditional_completed(&self.db, &job).await;
            let _ = self.event_journal.append(DomainEvent::file_change(&job.destination_connection_id, &job.destination_path, "upload"), Some(&job.id)).await;
            let _ = self.event_journal.append(DomainEvent::transfer_completed(&job), Some(&job.id)).await;
            let _ = self.completion_tx.send((job.id.clone(), true));
        }
        self.cancel_tokens.write().await.remove(job_id);
    }

    pub async fn fail_inline_job(&self, job_id: &str, err: String) {
        let job_opt = {
            let mut map = self.jobs.write().await;
            if let Some(j) = map.get_mut(job_id) {
                j.status = TransferStatus::Failed;
                j.error_message = Some(err.clone());
                j.speed_bytes_per_sec = 0;
                j.eta_seconds = None;
                j.updated_at = Utc::now();
                Some(j.clone())
            } else { None }
        };
        if let Some(job) = job_opt {
            let _ = Self::save_job_to_db(&self.db, &job).await;
            let _ = self.event_journal.append(DomainEvent::transfer_failed(&job), Some(&job.id)).await;
            let _ = self.completion_tx.send((job.id.clone(), false));
        }
        self.cancel_tokens.write().await.remove(job_id);
    }

    /// Retrieve list of transfer jobs filtered by authorization (P0 #4)
    pub async fn list_jobs(
        &self,
        user_id: Option<&str>,
        is_admin: bool,
        include_dismissed: bool,
    ) -> Vec<TransferJob> {
        if include_dismissed {
            if let Ok(saved) = Self::load_jobs_from_db(&self.db).await {
                let mut list: Vec<TransferJob> = saved
                    .into_iter()
                    .filter(|j| {
                        if is_admin {
                            return true;
                        }
                        match (&j.user_id, user_id) {
                            (Some(owner), Some(uid)) => owner == uid,
                            (None, _) => true,
                            _ => false,
                        }
                    })
                    .collect();
                list.sort_by_key(|b| std::cmp::Reverse(b.created_at));
                return list;
            }
        }

        let map = self.jobs.read().await;
        let mut list: Vec<TransferJob> = map
            .values()
            .filter(|j| {
                if !include_dismissed && j.dismissed_at.is_some() {
                    return false;
                }
                if is_admin {
                    return true;
                }
                match (&j.user_id, user_id) {
                    (Some(owner), Some(uid)) => owner == uid,
                    _ => false,
                }
            })
            .cloned()
            .collect();
        list.sort_by_key(|b| std::cmp::Reverse(b.created_at));
        list
    }

    pub async fn cancel_job(
        &self,
        id: &str,
        user_id: Option<&str>,
        is_admin: bool,
    ) -> Result<bool, String> {
        let (token_opt, is_queued, updated_job) = {
            let mut map = self.jobs.write().await;
            if let Some(job) = map.get_mut(id) {
                if !is_admin {
                    match (&job.user_id, user_id) {
                        (Some(owner), Some(uid)) if owner == uid => {}
                        _ => {
                            return Err(
                                "Permission denied: cannot cancel another user's transfer".into()
                            )
                        }
                    }
                }

                let token = self.cancel_tokens.read().await.get(id).cloned();

                if job.status == TransferStatus::Queued {
                    job.status = TransferStatus::Cancelled;
                    job.speed_bytes_per_sec = 0;
                    job.eta_seconds = None;
                    job.updated_at = Utc::now();
                    (token, true, Some(job.clone()))
                } else if job.status == TransferStatus::Running {
                    job.status = TransferStatus::CancellationRequested;
                    job.speed_bytes_per_sec = 0;
                    job.eta_seconds = None;
                    job.updated_at = Utc::now();
                    (token, false, Some(job.clone()))
                } else if job.status == TransferStatus::CancellationRequested {
                    (token, false, None)
                } else {
                    (None, false, None)
                }
            } else {
                (None, false, None)
            }
        };

        if let Some(token) = token_opt {
            token.cancel();
        }
        let _ = crate::transfer::checkpoint::TransferCheckpoint::delete(&self.db, id).await;

        if let Some(job) = updated_job {
            let _ = Self::save_job_to_db(&self.db, &job).await;
            if is_queued {
                let _ = self
                    .event_journal
                    .append(DomainEvent::transfer_failed(&job), Some(&job.id))
                    .await;
            } else {
                let _ = self
                    .event_journal
                    .append(DomainEvent::transfer_progress(&job), Some(&job.id))
                    .await;
            }
            Ok(true)
        } else {
            // DB Fallback if job is not in RAM
            if let Ok(Some(row)) =
                sqlx::query("SELECT user_id, status FROM transfer_jobs WHERE id = ?")
                    .bind(id)
                    .fetch_optional(&self.db)
                    .await
            {
                let db_user_id: Option<String> = row.try_get("user_id").ok();
                let db_status_str: String = row.try_get("status").unwrap_or_default();
                let db_status = TransferStatus::from_str(&db_status_str);

                if !is_admin {
                    match (&db_user_id, user_id) {
                        (Some(owner), Some(uid)) if owner == uid => {}
                        _ => {
                            return Err(
                                "Permission denied: cannot cancel another user's transfer".into()
                            )
                        }
                    }
                }

                if db_status == TransferStatus::Queued
                    || db_status == TransferStatus::Running
                    || db_status == TransferStatus::CancellationRequested
                {
                    let now_str = Utc::now().to_rfc3339();
                    let res = sqlx::query("UPDATE transfer_jobs SET status = 'cancelled', updated_at = ? WHERE id = ?")
                        .bind(&now_str)
                        .bind(id)
                        .execute(&self.db)
                        .await;
                    return Ok(res.map(|r| r.rows_affected() > 0).unwrap_or(false));
                }
            }
            Ok(false)
        }
    }

    /// Retry or resume an interrupted or failed transfer job.
    pub async fn retry_job(
        &self,
        id: &str,
        user_id: Option<&str>,
        is_admin: bool,
    ) -> Result<bool, String> {
        let (can_retry, job_opt) = {
            let mut map = self.jobs.write().await;
            if let Some(job) = map.get_mut(id) {
                if !is_admin {
                    match (&job.user_id, user_id) {
                        (Some(owner), Some(uid)) if owner == uid => {}
                        _ => {
                            return Err(
                                "Permission denied: cannot retry another user's transfer".into()
                            )
                        }
                    }
                }

                if matches!(
                    job.status,
                    TransferStatus::Failed | TransferStatus::Interrupted
                ) {
                    job.status = TransferStatus::Queued;
                    job.phase = TransferPhase::Preparing;
                    job.error_message = None;
                    job.updated_at = Utc::now();
                    (true, Some(job.clone()))
                } else {
                    (false, None)
                }
            } else {
                (false, None)
            }
        };

        if can_retry {
            if let Some(job) = job_opt {
                let _ = Self::save_job_to_db(&self.db, &job).await;
                let _ = self
                    .event_journal
                    .append(DomainEvent::transfer_progress(&job), Some(&job.id))
                    .await;
                let _ = self.queue_tx.send(id.to_string()).await;
                return Ok(true);
            }
        }

        Err(format!(
            "Transfer job '{}' cannot be retried (not in failed or interrupted state)",
            id
        ))
    }

    pub async fn dismiss_job(
        &self,
        id: &str,
        user_id: Option<&str>,
        is_admin: bool,
    ) -> Result<bool, String> {
        let updated_job = {
            let mut map = self.jobs.write().await;
            if let Some(job) = map.get_mut(id) {
                if !is_admin {
                    match (&job.user_id, user_id) {
                        (Some(owner), Some(uid)) if owner == uid => {}
                        _ => {
                            return Err(
                                "Permission denied: cannot dismiss another user's transfer".into()
                            )
                        }
                    }
                }
                let is_terminal = matches!(
                    job.status,
                    TransferStatus::Completed
                        | TransferStatus::Failed
                        | TransferStatus::Cancelled
                        | TransferStatus::Interrupted
                );
                if !is_terminal {
                    return Err(format!(
                        "Cannot dismiss active transfer '{}' (status: {}); cancel it first",
                        id,
                        job.status.as_str()
                    ));
                }
                job.dismissed_at = Some(Utc::now());
                job.updated_at = Utc::now();
                let j = job.clone();
                map.remove(id); // Evict dismissed job from RAM to bound memory
                Some(j)
            } else {
                None
            }
        };

        if let Some(job) = updated_job {
            let _ = Self::save_job_to_db(&self.db, &job).await;
            Ok(true)
        } else {
            // Check DB row exists and terminal before fallback update
            if let Ok(Some(r)) = sqlx::query("SELECT status FROM transfer_jobs WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.db)
                .await
            {
                use sqlx::Row;
                let st: String = r.try_get("status").unwrap_or_default();
                if !matches!(
                    st.as_str(),
                    "completed" | "failed" | "cancelled" | "interrupted"
                ) {
                    return Err(format!(
                        "Cannot dismiss active transfer '{}' (status: {}); cancel it first",
                        id, st
                    ));
                }
            }
            let now = Utc::now().to_rfc3339();
            let res = if is_admin {
                sqlx::query(
                    "UPDATE transfer_jobs SET dismissed_at = ?, updated_at = ? WHERE id = ? AND dismissed_at IS NULL AND status IN ('completed','failed','cancelled','interrupted')",
                )
                .bind(&now)
                .bind(&now)
                .bind(id)
                .execute(&self.db)
                .await
            } else if let Some(uid) = user_id {
                sqlx::query(
                    "UPDATE transfer_jobs SET dismissed_at = ?, updated_at = ? WHERE id = ? AND user_id = ? AND dismissed_at IS NULL AND status IN ('completed','failed','cancelled','interrupted')",
                )
                .bind(&now)
                .bind(&now)
                .bind(id)
                .bind(uid)
                .execute(&self.db)
                .await
            } else {
                return Err("Permission denied: cannot dismiss unowned transfer".into());
            };

            match res {
                Ok(r) => Ok(r.rows_affected() > 0),
                Err(e) => Err(e.to_string()),
            }
        }
    }

    /// Clear all finished (completed/failed/cancelled) transfers from memory and DB for current user
    pub async fn clear_finished_jobs(
        &self,
        user_id: Option<&str>,
        is_admin: bool,
    ) -> Result<usize, String> {
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let mut count = 0;
        let mut jobs_to_persist = Vec::new();

        {
            let mut map = self.jobs.write().await;
            let mut ids_to_evict = Vec::new();

            for (id, job) in map.iter_mut() {
                if job.dismissed_at.is_some() {
                    continue;
                }
                let is_finished = matches!(
                    job.status,
                    TransferStatus::Completed
                        | TransferStatus::Failed
                        | TransferStatus::Cancelled
                        | TransferStatus::Interrupted
                );
                if is_finished {
                    let can_clear = if is_admin {
                        true
                    } else {
                        match (&job.user_id, user_id) {
                            (Some(owner), Some(uid)) => owner == uid,
                            _ => false,
                        }
                    };

                    if can_clear {
                        job.dismissed_at = Some(now);
                        job.updated_at = now;
                        jobs_to_persist.push(job.clone());
                        ids_to_evict.push(id.clone());
                        count += 1;
                    }
                }
            }

            // Evict dismissed jobs from RAM
            for id in ids_to_evict {
                map.remove(&id);
            }
        }

        for j in &jobs_to_persist {
            let _ = Self::save_job_to_db(&self.db, j).await;
        }

        // Also update any finished jobs in DB that might have already been evicted from RAM and count them
        let db_extra: usize = if is_admin {
            sqlx::query(
                "UPDATE transfer_jobs SET dismissed_at = ?, updated_at = ? WHERE dismissed_at IS NULL AND status IN ('completed', 'failed', 'cancelled', 'interrupted')",
            )
            .bind(&now_str)
            .bind(&now_str)
            .execute(&self.db)
            .await
            .map(|r| r.rows_affected() as usize)
            .unwrap_or(0)
        } else if let Some(uid) = user_id {
            sqlx::query(
                "UPDATE transfer_jobs SET dismissed_at = ?, updated_at = ? WHERE dismissed_at IS NULL AND user_id = ? AND status IN ('completed', 'failed', 'cancelled', 'interrupted')",
            )
            .bind(&now_str)
            .bind(&now_str)
            .bind(uid)
            .execute(&self.db)
            .await
            .map(|r| r.rows_affected() as usize)
            .unwrap_or(0)
        } else {
            0
        };

        Ok(count + db_extra)
    }

    /// Execute transfer with exponential backoff retry for transient network hiccups (Dynamic retries P1 #17)
    #[allow(clippy::too_many_arguments)]
    async fn execute_job_with_retry(
        job: &mut TransferJob,
        cancel_token: &CancellationToken,
        providers: &Arc<RwLock<HashMap<String, Arc<dyn FileSystem>>>>,
        jobs_map: &Arc<RwLock<HashMap<String, TransferJob>>>,
        event_journal: &Arc<EventJournal>,
        db: &DbPool,
        max_retries: &Arc<AtomicUsize>,
    ) -> anyhow::Result<()> {
        let mut attempt = 0;

        loop {
            if cancel_token.is_cancelled() {
                return Err(anyhow::anyhow!("Transfer cancelled by user"));
            }

            attempt += 1;
            let max_attempts = max_retries.load(Ordering::Relaxed).max(1);
            match Self::execute_job(job, cancel_token, providers, jobs_map, event_journal, db).await
            {
                Ok(()) => return Ok(()),
                Err(e) => {
                    // Check if job was cancelled
                    if cancel_token.is_cancelled() {
                        return Err(anyhow::anyhow!("Transfer cancelled by user"));
                    }
                    {
                        let map = jobs_map.read().await;
                        if let Some(j) = map.get(&job.id) {
                            if j.status == TransferStatus::Cancelled
                                || j.status == TransferStatus::CancellationRequested
                            {
                                return Err(anyhow::anyhow!("Transfer cancelled by user"));
                            }
                        }
                    }

                    // Classify permanent errors vs retryable errors using typed policy
                    let is_retryable = crate::domain::RetryPolicy::is_anyhow_retryable(&e);
                    let retry_policy = crate::domain::RetryPolicy::new(max_attempts);
                    if !is_retryable || attempt >= max_attempts {
                        return Err(e);
                    }

                    let backoff = retry_policy.compute_backoff(attempt);
                    tracing::warn!(
                        "Transfer {} attempt {}/{} failed ({}), retrying in {:?}",
                        job.id,
                        attempt,
                        max_attempts,
                        e,
                        backoff
                    );

                    let cp = crate::transfer::checkpoint::TransferCheckpoint::load(db, &job.id)
                        .await
                        .ok()
                        .flatten();
                    let preserved_offset = cp.map(|c| c.offset).unwrap_or(job.transferred_bytes);

                    job.phase = TransferPhase::Preparing;
                    job.transferred_bytes = preserved_offset;
                    job.speed_bytes_per_sec = 0;
                    job.eta_seconds = None;
                    job.updated_at = Utc::now();
                    {
                        let mut map = jobs_map.write().await;
                        if let Some(j) = map.get_mut(&job.id) {
                            j.phase = TransferPhase::Preparing;
                            j.transferred_bytes = preserved_offset;
                            j.speed_bytes_per_sec = 0;
                            j.eta_seconds = None;
                            j.updated_at = Utc::now();
                        }
                    }
                    let _ = event_journal
                        .append(DomainEvent::transfer_progress(&job), Some(&job.id))
                        .await;

                    tokio::select! {
                        _ = cancel_token.cancelled() => {
                            return Err(anyhow::anyhow!("Transfer cancelled by user"));
                        }
                        _ = tokio::time::sleep(backoff) => {}
                    }
                }
            }
        }
    }

    /// True Bounded-Buffer Asynchronous Streaming Transfer with SHA-256 Checksum Calculation & Instant CancellationToken Abort
    #[allow(clippy::too_many_arguments)]
    pub async fn execute_job(
        job: &mut TransferJob,
        cancel_token: &CancellationToken,
        providers: &Arc<RwLock<HashMap<String, Arc<dyn FileSystem>>>>,
        jobs_map: &Arc<RwLock<HashMap<String, TransferJob>>>,
        event_journal: &Arc<EventJournal>,
        db: &DbPool,
    ) -> anyhow::Result<()> {
        if cancel_token.is_cancelled() {
            return Err(anyhow::anyhow!("Transfer cancelled by user"));
        }

        let src_fs = {
            let p = providers.read().await;
            p.get(&job.source_connection_id).cloned().ok_or_else(|| {
                anyhow::anyhow!("Source connection '{}' not found", job.source_connection_id)
            })?
        };

        let dst_fs = {
            let p = providers.read().await;
            p.get(&job.destination_connection_id)
                .cloned()
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Destination connection '{}' not found",
                        job.destination_connection_id
                    )
                })?
        };

        let src_vfs = VfsPath::new(&job.source_connection_id, &job.source_path)?;
        let dst_vfs = VfsPath::new(&job.destination_connection_id, &job.destination_path)?;

        job.phase = TransferPhase::Preparing;

        // If a Move operation previously completed copying and verified destination,
        // but failed during source cleanup, complete source cleanup directly.
        if job.transfer_type == TransferType::Move && job.phase == TransferPhase::CleaningUp {
            let _ = src_fs.delete(&src_vfs).await;
            job.phase = TransferPhase::Completed;
            job.transferred_bytes = job.total_bytes;
            job.speed_bytes_per_sec = 0;
            job.eta_seconds = Some(0);
            job.updated_at = Utc::now();
            return Ok(());
        }

        // 1. Get source metadata
        let meta = src_fs
            .stat(&src_vfs)
            .await
            .map_err(|e| anyhow::anyhow!("Stat source failed: {}", e))?;

        if meta.kind != crate::domain::FileKind::Directory {
            job.total_bytes = meta.size;
            {
                let mut map = jobs_map.write().await;
                if let Some(j) = map.get_mut(&job.id) {
                    j.total_bytes = meta.size;
                    j.phase = TransferPhase::Preparing;
                }
            }
            let _ = event_journal
                .append(DomainEvent::transfer_progress(&job), Some(&job.id))
                .await;
        }

        let strategy = TransferPlanner::plan_transfer(job, &src_fs, &dst_fs, &src_vfs, &dst_vfs);

        match strategy {
            TransferStrategy::NativeRename => {
                if cancel_token.is_cancelled() {
                    return Err(anyhow::anyhow!("Transfer cancelled by user"));
                }
                let rename_fut = src_fs.rename(&src_vfs, &dst_vfs);
                let rename_res = tokio::select! {
                    _ = cancel_token.cancelled() => {
                        return Err(anyhow::anyhow!("Transfer cancelled by user"));
                    }
                    res = rename_fut => res,
                };
                match rename_res {
                    Ok(_) => {
                        if cancel_token.is_cancelled() {
                            return Err(anyhow::anyhow!("Transfer cancelled by user"));
                        }
                        job.transferred_bytes = meta.size;
                        job.total_bytes = meta.size;
                        job.phase = TransferPhase::Completed;
                        job.speed_bytes_per_sec = 0;
                        job.eta_seconds = Some(0);
                        job.updated_at = Utc::now();
                        return Ok(());
                    }
                    Err(e) => {
                        tracing::warn!("Native rename fallback on same-connection move ({}), streaming copy+delete", e);
                    }
                }
            }
            TransferStrategy::ServerSideCopy => {
                if cancel_token.is_cancelled() {
                    return Err(anyhow::anyhow!("Transfer cancelled by user"));
                }
                let copy_fut = src_fs.copy(&src_vfs, &dst_vfs);
                let copy_res = tokio::select! {
                    _ = cancel_token.cancelled() => {
                        let _ = dst_fs.delete(&dst_vfs).await;
                        return Err(anyhow::anyhow!("Transfer cancelled by user"));
                    }
                    res = copy_fut => res,
                };
                match copy_res {
                    Ok(_) => {
                        if cancel_token.is_cancelled() {
                            let _ = dst_fs.delete(&dst_vfs).await;
                            return Err(anyhow::anyhow!("Transfer cancelled by user"));
                        }
                        if job.transfer_type == TransferType::Move {
                            src_fs.delete(&src_vfs).await.map_err(|e| {
                                anyhow::anyhow!(
                                    "Cleanup source failed during server-side move: {}",
                                    e
                                )
                            })?;
                        }
                        job.transferred_bytes = meta.size;
                        job.total_bytes = meta.size;
                        job.checksum = Some(meta.etag.clone());
                        job.phase = TransferPhase::Completed;
                        job.speed_bytes_per_sec = 0;
                        job.eta_seconds = Some(0);
                        job.updated_at = Utc::now();
                        return Ok(());
                    }
                    Err(e) => {
                        tracing::warn!("Server-side copy fallback ({}), streaming data", e);
                    }
                }
            }
            TransferStrategy::Streaming => {}
        }

        if meta.kind == crate::domain::FileKind::Directory {
            const MAX_TRANSFER_DIR_ENTRIES: usize = 100_000;
            const MAX_TRANSFER_DIR_DEPTH: usize = 64;

            #[derive(Debug, Clone)]
            struct ItemToTransfer {
                rel_path: String,
                is_dir: bool,
                size: u64,
            }

            // Producer-Consumer Bounded Directory Scanner: Memory bounded to O(channel_capacity)
            let (tx, mut rx) = tokio::sync::mpsc::channel::<ItemToTransfer>(256);
            let src_fs_clone = Arc::clone(&src_fs);
            let cancel_token_clone = cancel_token.clone();
            let src_conn_id = job.source_connection_id.clone();
            let base_vfs = src_vfs.clone();

            let scanner_handle = tokio::spawn(async move {
                async fn scan_recursive(
                    fs: &Arc<dyn FileSystem>,
                    cancel_token: &CancellationToken,
                    conn_id: &str,
                    base_vfs: &VfsPath,
                    current_rel: &str,
                    depth: usize,
                    tx: &tokio::sync::mpsc::Sender<ItemToTransfer>,
                    count: &mut usize,
                ) -> anyhow::Result<()> {
                    if cancel_token.is_cancelled() {
                        return Err(anyhow::anyhow!("Transfer cancelled by user"));
                    }
                    if depth > MAX_TRANSFER_DIR_DEPTH {
                        return Err(anyhow::anyhow!(
                            "Directory recursion depth limit exceeded (max {})",
                            MAX_TRANSFER_DIR_DEPTH
                        ));
                    }
                    if *count >= MAX_TRANSFER_DIR_ENTRIES {
                        return Err(anyhow::anyhow!(
                            "Directory entries count limit exceeded (max {})",
                            MAX_TRANSFER_DIR_ENTRIES
                        ));
                    }

                    let current_vfs = if current_rel.is_empty() {
                        base_vfs.clone()
                    } else {
                        VfsPath::new(
                            conn_id,
                            format!("{}/{}", base_vfs.path.trim_end_matches('/'), current_rel),
                        )?
                    };

                    use futures::StreamExt;
                    let mut stream = fs
                        .list_stream(&current_vfs)
                        .await
                        .map_err(|e| anyhow::anyhow!("List stream failed: {}", e))?;

                    while let Some(entry_res) = stream.next().await {
                        let entry = entry_res.map_err(|e| anyhow::anyhow!("List error: {}", e))?;
                        if cancel_token.is_cancelled() {
                            return Err(anyhow::anyhow!("Transfer cancelled by user"));
                        }
                        let child_rel = if current_rel.is_empty() {
                            entry.name.clone()
                        } else {
                            format!("{}/{}", current_rel, entry.name)
                        };

                        *count += 1;
                        if entry.kind == crate::domain::FileKind::Directory {
                            tx.send(ItemToTransfer {
                                rel_path: child_rel.clone(),
                                is_dir: true,
                                size: 0,
                            })
                            .await
                            .map_err(|_| anyhow::anyhow!("Scanner channel closed"))?;

                            Box::pin(scan_recursive(
                                fs,
                                cancel_token,
                                conn_id,
                                base_vfs,
                                &child_rel,
                                depth + 1,
                                tx,
                                count,
                            ))
                            .await?;
                        } else {
                            tx.send(ItemToTransfer {
                                rel_path: child_rel,
                                is_dir: false,
                                size: entry.size.unwrap_or(0),
                            })
                            .await
                            .map_err(|_| anyhow::anyhow!("Scanner channel closed"))?;
                        }
                    }
                    Ok(())
                }

                let mut count = 0;
                scan_recursive(
                    &src_fs_clone,
                    &cancel_token_clone,
                    &src_conn_id,
                    &base_vfs,
                    "",
                    0,
                    &tx,
                    &mut count,
                )
                .await
            });

            // 1. Create root destination directory
            if let Err(e) = dst_fs.create_dir(&dst_vfs).await {
                if !dst_fs
                    .stat(&dst_vfs)
                    .await
                    .map(|m| m.kind == crate::domain::FileKind::Directory)
                    .unwrap_or(false)
                {
                    // No ticker yet at this point, no need to abort
                    return Err(anyhow::anyhow!(
                        "Failed creating root destination directory '{}': {}",
                        dst_vfs.path,
                        e
                    ));
                }
            }

            // Emit immediate Transferring 0% phase transition
            job.phase = TransferPhase::Transferring;
            {
                let mut map = jobs_map.write().await;
                if let Some(j) = map.get_mut(&job.id) {
                    j.phase = TransferPhase::Transferring;
                    j.updated_at = Utc::now();
                }
            }
            let _ = event_journal
                .append(DomainEvent::transfer_progress(&job), Some(&job.id))
                .await;

            let concurrency = if dst_fs.capabilities().write_can_multi {
                8
            } else {
                4
            };
            let (file_tx, file_rx) = tokio::sync::mpsc::channel::<ItemToTransfer>(128);

            let transferred_atomic = Arc::new(AtomicU64::new(0));
            let total_bytes_atomic = Arc::new(AtomicU64::new(0));
            let start_time = Instant::now();

            let worker_src_fs = Arc::clone(&src_fs);
            let worker_dst_fs = Arc::clone(&dst_fs);
            let worker_src_vfs = src_vfs.clone();
            let worker_dst_vfs = dst_vfs.clone();
            let worker_src_conn = job.source_connection_id.clone();
            let worker_dst_conn = job.destination_connection_id.clone();
            let worker_is_move = job.transfer_type == TransferType::Move;
            let worker_cancel_token = cancel_token.clone();
            let worker_transferred = Arc::clone(&transferred_atomic);

            // Spawn background progress ticker (every 100ms) for real-time byte progress during directory transfers
            let ticker_cancel = cancel_token.clone();
            let ticker_transferred = Arc::clone(&transferred_atomic);
            let ticker_total = Arc::clone(&total_bytes_atomic);
            let ticker_job_id = job.id.clone();
            let ticker_jobs_map = Arc::clone(jobs_map);
            let ticker_journal = Arc::clone(event_journal);
            let ticker_start = start_time;

            let ticker_handle = tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_millis(100));
                loop {
                    tokio::select! {
                        _ = ticker_cancel.cancelled() => break,
                        _ = interval.tick() => {
                            let current_bytes = ticker_transferred.load(Ordering::Relaxed);
                            let total = ticker_total.load(Ordering::Relaxed);
                            let elapsed_secs = ticker_start.elapsed().as_secs_f64().max(0.001);
                            let speed = (current_bytes as f64 / elapsed_secs) as u64;
                            let eta = if speed > 0 && total > current_bytes {
                                Some((total - current_bytes) / speed)
                            } else {
                                Some(0)
                            };
                            let updated = {
                                let mut map = ticker_jobs_map.write().await;
                                if let Some(j) = map.get_mut(&ticker_job_id) {
                                    if j.status == TransferStatus::Cancelled
                                        || j.status == TransferStatus::CancellationRequested
                                        || ticker_cancel.is_cancelled()
                                    {
                                        None
                                    } else {
                                        j.transferred_bytes = current_bytes;
                                        j.total_bytes = total;
                                        j.speed_bytes_per_sec = speed;
                                        j.eta_seconds = eta;
                                        j.phase = TransferPhase::Transferring;
                                        j.updated_at = Utc::now();
                                        Some(j.clone())
                                    }
                                } else {
                                    None
                                }
                            };
                            if let Some(j) = updated {
                                let _ = ticker_journal.append(DomainEvent::transfer_progress(&j), Some(&ticker_job_id)).await;
                            }
                        }
                    }
                }
            });

            let file_workers = tokio::spawn(async move {
                use futures::StreamExt;
                let rx_stream = futures::stream::unfold(file_rx, |mut rx| async move {
                    rx.recv().await.map(|item| (item, rx))
                });
                let transfer_stream = rx_stream.map(|item| {
                    let src_fs = Arc::clone(&worker_src_fs);
                    let dst_fs = Arc::clone(&worker_dst_fs);
                    let src_vfs = worker_src_vfs.clone();
                    let dst_vfs = worker_dst_vfs.clone();
                    let src_conn_id = worker_src_conn.clone();
                    let dst_conn_id = worker_dst_conn.clone();
                    let is_move = worker_is_move;
                    let cancel_token = worker_cancel_token.clone();
                    let transferred_atomic = Arc::clone(&worker_transferred);

                    async move {
                        if cancel_token.is_cancelled() {
                            return Err(anyhow::anyhow!("Transfer cancelled by user"));
                        }

                        let src_file_vfs = VfsPath::new(
                            &src_conn_id,
                            format!("{}/{}", src_vfs.path.trim_end_matches('/'), item.rel_path),
                        )?;
                        let dst_file_vfs = VfsPath::new(
                            &dst_conn_id,
                            format!("{}/{}", dst_vfs.path.trim_end_matches('/'), item.rel_path),
                        )?;

                        let mut reader = src_fs
                            .read_stream(&src_file_vfs)
                            .await
                            .map_err(|e| anyhow::anyhow!("Read failed: {}", e))?;
                        let (mut pipe_writer, pipe_reader) = tokio::io::duplex(64 * 1024);
                        let cancel_pump = cancel_token.clone();
                        let transferred_atomic_pump = Arc::clone(&transferred_atomic);

                        let pump_handle = tokio::spawn(async move {
                            let mut buffer = vec![0u8; 64 * 1024];
                            let mut file_transferred = 0u64;
                            loop {
                                if cancel_pump.is_cancelled() {
                                    return Err(anyhow::anyhow!("Transfer cancelled by user"));
                                }
                                let n = tokio::select! {
                                    _ = cancel_pump.cancelled() => return Err(anyhow::anyhow!("Transfer cancelled by user")),
                                    res = reader.read(&mut buffer) => res?,
                                };
                                if n == 0 {
                                    break;
                                }
                                tokio::select! {
                                    _ = cancel_pump.cancelled() => return Err(anyhow::anyhow!("Transfer cancelled by user")),
                                    res = pipe_writer.write_all(&buffer[..n]) => res?,
                                };
                                file_transferred += n as u64;
                                transferred_atomic_pump.fetch_add(n as u64, Ordering::Relaxed);
                            }
                            pipe_writer.flush().await?;
                            drop(pipe_writer);
                            Ok::<u64, anyhow::Error>(file_transferred)
                        });

                        let write_fut = dst_fs.write_stream(&dst_file_vfs, Box::new(pipe_reader));
                        tokio::select! {
                            _ = cancel_token.cancelled() => {
                                let _ = dst_fs.delete(&dst_file_vfs).await;
                                return Err(anyhow::anyhow!("Transfer cancelled by user"));
                            }
                            res = write_fut => res.map_err(|e| anyhow::anyhow!("Write failed: {}", e))?,
                        }

                        let _file_bytes = pump_handle.await.map_err(|e| anyhow::anyhow!("Pump panic: {}", e))??;

                        if is_move {
                            src_fs.delete(&src_file_vfs).await.map_err(|e| {
                                anyhow::anyhow!("Failed to delete source file {}: {}", src_file_vfs.path, e)
                            })?;
                        }

                        Ok::<(), anyhow::Error>(())
                    }
                });

                let buffered_stream = transfer_stream.buffer_unordered(concurrency);
                futures::pin_mut!(buffered_stream);
                while let Some(res) = buffered_stream.next().await {
                    res?;
                }
                Ok::<(), anyhow::Error>(())
            });

            // Pump scanner items directly into directory creators and file queue
            while let Some(item) = rx.recv().await {
                if cancel_token.is_cancelled() {
                    ticker_handle.abort();
                    return Err(anyhow::anyhow!("Transfer cancelled by user"));
                }
                if item.is_dir {
                    let dst_dir_vfs = VfsPath::new(
                        &job.destination_connection_id,
                        format!("{}/{}", dst_vfs.path.trim_end_matches('/'), item.rel_path),
                    )?;
                    if let Err(e) = dst_fs.create_dir(&dst_dir_vfs).await {
                        if !dst_fs
                            .stat(&dst_dir_vfs)
                            .await
                            .map(|m| m.kind == crate::domain::FileKind::Directory)
                            .unwrap_or(false)
                        {
                            ticker_handle.abort();
                            return Err(anyhow::anyhow!(
                                "Failed creating subdirectory '{}': {}",
                                dst_dir_vfs.path,
                                e
                            ));
                        }
                    }
                } else {
                    total_bytes_atomic.fetch_add(item.size, Ordering::SeqCst);
                    tokio::select! {
                        _ = cancel_token.cancelled() => {
                            ticker_handle.abort();
                            return Err(anyhow::anyhow!("Transfer cancelled by user"));
                        },
                        res = file_tx.send(item) => {
                            res.map_err(|_| {
                                ticker_handle.abort();
                                anyhow::anyhow!("File worker channel closed")
                            })?;
                        }
                    }
                }
            }

            drop(file_tx); // Signal end of files to workers

            let scanner_res = scanner_handle
                .await
                .map_err(|e| anyhow::anyhow!("Scanner panic: {}", e))?;
            if let Err(e) = scanner_res {
                ticker_handle.abort();
                return Err(e);
            }

            let workers_res = file_workers
                .await
                .map_err(|e| anyhow::anyhow!("Worker pool panic: {}", e))?;
            if let Err(e) = workers_res {
                ticker_handle.abort();
                return Err(e);
            }

            // Stop background directory ticker
            ticker_handle.abort();

            // Move transfer: remove source directory after empty
            if job.transfer_type == TransferType::Move {
                job.phase = TransferPhase::CleaningUp;
                {
                    let mut map = jobs_map.write().await;
                    if let Some(j) = map.get_mut(&job.id) {
                        j.phase = TransferPhase::CleaningUp;
                        j.updated_at = Utc::now();
                    }
                }
                let _ = event_journal
                    .append(DomainEvent::transfer_progress(&job), Some(&job.id))
                    .await;

                src_fs.delete(&src_vfs).await.map_err(|e| {
                    anyhow::anyhow!("Failed to delete source directory {}: {}", src_vfs.path, e)
                })?;
            }

            job.transferred_bytes = total_bytes_atomic.load(Ordering::SeqCst);
            job.total_bytes = total_bytes_atomic.load(Ordering::SeqCst);
            job.speed_bytes_per_sec = 0;
            job.eta_seconds = Some(0);
            job.updated_at = Utc::now();
            return Ok(());
        }

        // Single File Transfer with In-Flight Checksum Calculation
        let total_bytes = job.total_bytes;

        // 1. Resolve destination permissions according to inheritance policy
        if let Some(perms) = crate::domain::resolve_destination_permissions(
            &dst_fs,
            &dst_vfs,
            false,
            crate::domain::PermissionInheritanceMode::InheritExistingOrParent,
        )
        .await
        {
            let _ = dst_fs.set_permissions(&dst_vfs, &perms).await;
        }

        // 2. Determine staging via TransferPlan / job.staging (single source of truth).
        // Engine must NOT re-decide via capabilities; job.staging comes from TransferPlanner::plan_upload.
        let tmp_plan = crate::transfer::plan::TransferPlan {
            execution_mode: job.execution_mode,
            staging: job.staging,
            commit: if job.staging == crate::transfer::TransferStaging::LocalTemp {
                crate::domain::CommitSemantics::AtomicRename
            } else if job.staging == crate::transfer::TransferStaging::ProviderTemp {
                crate::domain::CommitSemantics::AtomicObjectPut
            } else {
                crate::domain::CommitSemantics::DirectWrite
            },
        };
        let use_staging = tmp_plan.uses_staging();
        let staging_path = tmp_plan
            .staging_path(&dst_vfs, &job.id)
            .map(|v| v.path)
            .unwrap_or_default();
        let write_target_vfs = if use_staging {
            VfsPath::new(&job.destination_connection_id, staging_path.clone())?
        } else {
            dst_vfs.clone()
        };

        // 3. Safe Resume Verification: Destination MUST support append, range_read MUST be available,
        // and destination target file MUST already exist with exact matching size.
        let can_append = dst_fs.capabilities().write_can_append;
        let can_range_read = src_fs.capabilities().range_read;
        let src_meta = src_fs.stat(&src_vfs).await.ok();

        // Check if there is a saved checkpoint and whether source etag changed
        let saved_checkpoint = crate::transfer::checkpoint::TransferCheckpoint::load(db, &job.id)
            .await
            .ok()
            .flatten();
        let checkpoint_valid = if let Some(cp) = &saved_checkpoint {
            if let (Some(saved_etag), Some(src)) = (&cp.source_etag, src_meta.as_ref()) {
                if saved_etag != &src.etag {
                    tracing::warn!(
                        "transfer.resume: Source ETag changed since checkpoint, restarting from 0"
                    );
                    false
                } else {
                    true
                }
            } else {
                true
            }
        } else {
            true
        };

        let target_offset = saved_checkpoint
            .as_ref()
            .map(|cp| cp.offset)
            .unwrap_or(job.transferred_bytes);

        let existing_part_stat = if can_append
            && can_range_read
            && checkpoint_valid
            && target_offset > 0
            && target_offset < total_bytes
        {
            dst_fs.stat(&write_target_vfs).await.ok()
        } else {
            None
        };

        let resume_offset = if let Some(part_meta) = existing_part_stat {
            if part_meta.size == target_offset {
                target_offset
            } else {
                // Target file size does not match recorded progress; clean restart for safety
                let _ = dst_fs.delete(&write_target_vfs).await;
                let _ = crate::transfer::checkpoint::TransferCheckpoint::delete(db, &job.id).await;
                0
            }
        } else {
            // Target file missing or append unsupported; clean restart
            if use_staging {
                let _ = dst_fs.delete(&write_target_vfs).await;
            }
            0
        };

        job.transferred_bytes = resume_offset;

        let mut reader = if resume_offset > 0 {
            tracing::info!(
                "transfer.resume: job_id={} offset={} total={}",
                job.id,
                resume_offset,
                total_bytes
            );
            src_fs
                .read_range(
                    &src_vfs,
                    resume_offset,
                    total_bytes.saturating_sub(resume_offset),
                )
                .await
                .map_err(|e| anyhow::anyhow!("Read range stream failed: {}", e))?
        } else {
            src_fs
                .read_stream(&src_vfs)
                .await
                .map_err(|e| anyhow::anyhow!("Read stream failed: {}", e))?
        };

        // 4. Create a 64 KB bounded duplex async pipe (Zero huge RAM allocations)
        let (mut pipe_writer, pipe_reader) = tokio::io::duplex(64 * 1024);

        let job_id = job.id.clone();
        let jobs_map_clone = Arc::clone(jobs_map);
        let event_journal_clone = Arc::clone(event_journal);
        let db_clone = db.clone();
        let cancel_token_pump = cancel_token.clone();
        let staging_path_clone = staging_path.clone();
        let source_etag_clone = src_meta.map(|m| m.etag);

        // Emit immediate Transferring 0% phase transition
        job.phase = TransferPhase::Transferring;
        {
            let mut map = jobs_map.write().await;
            if let Some(j) = map.get_mut(&job.id) {
                j.phase = TransferPhase::Transferring;
                j.updated_at = Utc::now();
            }
        }
        let _ = event_journal
            .append(DomainEvent::transfer_progress(&job), Some(&job.id))
            .await;

        // 5. Spawn writer task to pump data, calculate SHA-256 checksum on-the-fly, and write to pipe
        let is_clean_start = resume_offset == 0;
        let pump_handle = tokio::spawn(async move {
            let mut buffer = vec![0u8; 64 * 1024];
            let mut transferred = resume_offset;
            let mut hasher = Sha256::new();
            let start_time = Instant::now();
            let mut last_emit = Instant::now();
            let mut last_db_save = Instant::now();

            loop {
                if cancel_token_pump.is_cancelled() {
                    return Err(anyhow::anyhow!("Transfer cancelled by user"));
                }

                let n = tokio::select! {
                    _ = cancel_token_pump.cancelled() => {
                        return Err(anyhow::anyhow!("Transfer cancelled by user"));
                    }
                    res = reader.read(&mut buffer) => res?,
                };

                if n == 0 {
                    break;
                }

                if is_clean_start {
                    hasher.update(&buffer[..n]);
                }

                // Write chunk into bounded pipe (awaits if destination consumer is slower)
                tokio::select! {
                    _ = cancel_token_pump.cancelled() => {
                        return Err(anyhow::anyhow!("Transfer cancelled by user"));
                    }
                    res = pipe_writer.write_all(&buffer[..n]) => res?,
                };

                transferred += n as u64;

                let now = Instant::now();
                if now.duration_since(last_emit) >= Duration::from_millis(100)
                    || transferred == total_bytes
                {
                    last_emit = now;
                    let elapsed_sec = start_time.elapsed().as_secs_f64();
                    let bytes_since_start = transferred.saturating_sub(resume_offset);
                    let speed = if elapsed_sec > 0.05 {
                        (bytes_since_start as f64 / elapsed_sec) as u64
                    } else {
                        0
                    };
                    let eta = if speed > 0 && total_bytes > transferred {
                        Some((total_bytes - transferred) / speed)
                    } else {
                        Some(0)
                    };

                    let updated_job = {
                        let mut map = jobs_map_clone.write().await;
                        if let Some(j) = map.get_mut(&job_id) {
                            if j.status == TransferStatus::Cancelled
                                || j.status == TransferStatus::CancellationRequested
                                || cancel_token_pump.is_cancelled()
                            {
                                None
                            } else {
                                j.transferred_bytes = transferred;
                                j.total_bytes = total_bytes;
                                j.speed_bytes_per_sec = speed;
                                j.eta_seconds = eta;
                                j.phase = if transferred >= total_bytes && total_bytes > 0 {
                                    TransferPhase::Finalizing
                                } else {
                                    TransferPhase::Transferring
                                };
                                j.updated_at = Utc::now();
                                Some(j.clone())
                            }
                        } else {
                            None
                        }
                    };

                    if let Some(j) = updated_job {
                        let _ = event_journal_clone
                            .append(DomainEvent::transfer_progress(&j), Some(&job_id))
                            .await;

                        // Persist to DB every 2 seconds or on finish
                        if last_db_save.elapsed().as_secs() >= 2 || transferred == total_bytes {
                            let _ = Self::save_job_to_db(&db_clone, &j).await;
                            let cp = crate::transfer::checkpoint::TransferCheckpoint {
                                transfer_id: job_id.clone(),
                                offset: transferred,
                                total: total_bytes,
                                staging_path: staging_path_clone.clone(),
                                source_etag: source_etag_clone.clone(),
                                source_version: None,
                                checksum_so_far: None,
                                updated_at: Utc::now(),
                            };
                            let _ = cp.save(&db_clone).await;
                            last_db_save = Instant::now();
                        }
                    }
                }
            }

            pipe_writer.flush().await?;
            drop(pipe_writer); // Signal EOF to destination consumer

            // Emit Finalizing phase explicitly to UI ONLY if not cancelled
            let emit_finalizing = {
                let mut map = jobs_map_clone.write().await;
                if let Some(j) = map.get_mut(&job_id) {
                    if j.status != TransferStatus::Cancelled
                        && j.status != TransferStatus::CancellationRequested
                        && !cancel_token_pump.is_cancelled()
                    {
                        j.transferred_bytes = transferred;
                        j.phase = TransferPhase::Finalizing;
                        j.speed_bytes_per_sec = 0;
                        j.eta_seconds = Some(0);
                        j.updated_at = Utc::now();
                        Some(j.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            };
            if let Some(j) = emit_finalizing {
                let _ = event_journal_clone
                    .append(DomainEvent::transfer_progress(&j), Some(&job_id))
                    .await;
            }

            let checksum_hex = if is_clean_start {
                Some(hex::encode(hasher.finalize()))
            } else {
                None
            };
            Ok::<(u64, Option<String>), anyhow::Error>((transferred, checksum_hex))
        });

        // 6. Destination writes into target file with joint cancellation synchronization
        let write_fut = dst_fs.write_stream(&write_target_vfs, Box::new(pipe_reader));
        let mut pump_handle = pump_handle;
        let (write_res, pump_res) = tokio::select! {
            _ = cancel_token.cancelled() => {
                pump_handle.abort();
                let _ = pump_handle.await;
                let _ = dst_fs.delete(&write_target_vfs).await;
                return Err(anyhow::anyhow!("Transfer cancelled by user"));
            }
            (w, p) = async {
                let w_res = write_fut.await;
                let p_res = (&mut pump_handle).await;
                (w_res, p_res)
            } => {
                let p_val = match p {
                    Ok(res) => res,
                    Err(e) if e.is_cancelled() => Err(anyhow::anyhow!("Transfer cancelled by user")),
                    Err(e) => Err(anyhow::anyhow!("Stream pump task panicked: {}", e)),
                };
                (w, p_val)
            }
        };

        let (transferred_bytes, checksum) = match pump_res {
            Ok(val) => val,
            Err(e) => {
                let _ = dst_fs.delete(&write_target_vfs).await;
                return Err(e);
            }
        };

        if let Err(e) = write_res {
            let _ = dst_fs.delete(&write_target_vfs).await;
            return Err(anyhow::anyhow!("Destination write failed: {}", e));
        }

        // Atomically promote part file to destination path ONLY if staging was used
        if use_staging {
            if let Err(e) = dst_fs.rename(&write_target_vfs, &dst_vfs).await {
                let _ = dst_fs.delete(&write_target_vfs).await;
                return Err(anyhow::anyhow!(
                    "Failed to promote staging file to final destination '{}': {}",
                    dst_vfs.path,
                    e
                ));
            }
        }

        if let Some(perms) = crate::domain::resolve_destination_permissions(
            &dst_fs,
            &dst_vfs,
            false,
            crate::domain::PermissionInheritanceMode::InheritExistingOrParent,
        )
        .await
        {
            let _ = dst_fs.set_permissions(&dst_vfs, &perms).await;
        }

        job.transferred_bytes = transferred_bytes;
        job.checksum = checksum.clone();

        // 6. Verification Phase
        job.phase = TransferPhase::Verifying;
        {
            let mut map = jobs_map.write().await;
            if let Some(j) = map.get_mut(&job.id) {
                j.phase = TransferPhase::Verifying;
                j.transferred_bytes = transferred_bytes;
                j.checksum = checksum;
                j.updated_at = Utc::now();
            }
        }
        let _ = event_journal
            .append(DomainEvent::transfer_progress(&job), Some(&job.id))
            .await;

        let dst_stat = dst_fs
            .stat(&dst_vfs)
            .await
            .map_err(|e| anyhow::anyhow!("Destination verification failed: {}", e))?;
        if dst_stat.size != job.total_bytes && job.total_bytes > 0 {
            return Err(anyhow::anyhow!(
                "Integrity check failed: destination size ({}) does not match source size ({})",
                dst_stat.size,
                job.total_bytes
            ));
        }

        // 7. Transactional Move: delete source after full verification
        if job.transfer_type == TransferType::Move {
            job.phase = TransferPhase::CleaningUp;
            {
                let mut map = jobs_map.write().await;
                if let Some(j) = map.get_mut(&job.id) {
                    j.phase = TransferPhase::CleaningUp;
                    j.updated_at = Utc::now();
                }
            }
            let _ = event_journal
                .append(DomainEvent::transfer_progress(&job), Some(&job.id))
                .await;

            // Safely delete source file
            src_fs
                .delete(&src_vfs)
                .await
                .map_err(|e| anyhow::anyhow!("Move completed with cleanup failure: {}", e))?;
        }

        job.phase = TransferPhase::Completed;
        job.speed_bytes_per_sec = 0;
        job.eta_seconds = Some(0);
        job.updated_at = Utc::now();

        Ok(())
    }

    /// Load transfer jobs from SQLite
    async fn load_jobs_from_db(db: &DbPool) -> anyhow::Result<Vec<TransferJob>> {
        let rows = sqlx::query(
            "SELECT id, user_id, name, transfer_type, source_connection_id, source_path,
                    destination_connection_id, destination_path, status, phase,
                    transferred_bytes, total_bytes, speed_bytes_per_sec,
                    eta_seconds, checksum, error_message, dismissed_at, created_at, updated_at
             FROM transfer_jobs
             ORDER BY created_at DESC
             LIMIT 100",
        )
        .fetch_all(db)
        .await?;

        let mut jobs = Vec::new();
        for r in rows {
            let id: String = r.get("id");
            let user_id: Option<String> = r.get("user_id");
            let name: String = r.get("name");
            let transfer_type_str: String = r.get("transfer_type");
            let source_connection_id: String = r.get("source_connection_id");
            let source_path: String = r.get("source_path");
            let destination_connection_id: String = r.get("destination_connection_id");
            let destination_path: String = r.get("destination_path");
            let status_str: String = r.get("status");
            let phase_str: Option<String> = r.try_get("phase").ok();
            let transferred_bytes: i64 = r.get("transferred_bytes");
            let total_bytes: i64 = r.get("total_bytes");
            let speed_bytes_per_sec: i64 = r.get("speed_bytes_per_sec");
            let eta_seconds: Option<i64> = r.get("eta_seconds");
            let checksum: Option<String> = r.get("checksum");
            let error_message: Option<String> = r.get("error_message");
            let dismissed_at_str: Option<String> = r.get("dismissed_at");
            let created_at_str: String = r.get("created_at");
            let updated_at_str: String = r.get("updated_at");

            let dismissed_at = dismissed_at_str.and_then(|d| {
                DateTime::parse_from_rfc3339(&d)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc))
            });
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            let phase = phase_str
                .as_deref()
                .map(TransferPhase::from_str)
                .unwrap_or(TransferPhase::Preparing);

            let execution_mode: String = r.try_get("execution_mode").unwrap_or_else(|_| "inline".to_string());
            let staging: String = r.try_get("staging").unwrap_or_else(|_| "none".to_string());
            jobs.push(TransferJob {
                id,
                user_id,
                name,
                transfer_type: TransferType::from_str(&transfer_type_str),
                source_connection_id,
                source_path,
                destination_connection_id,
                destination_path,
                status: TransferStatus::from_str(&status_str),
                phase,
                execution_mode: TransferExecutionMode::from_str(&execution_mode),
                staging: TransferStaging::from_str(&staging),
                transferred_bytes: transferred_bytes as u64,
                total_bytes: total_bytes as u64,
                speed_bytes_per_sec: speed_bytes_per_sec as u64,
                eta_seconds: eta_seconds.map(|e| e as u64),
                checksum,
                error_message,
                dismissed_at,
                created_at,
                updated_at,
            });
        }

        Ok(jobs)
    }

    /// Conditional save: do not overwrite cancelled status with completed (race guard)
    async fn save_job_conditional_completed(db: &DbPool, job: &TransferJob) -> anyhow::Result<()> {
        let created_at = job.created_at.to_rfc3339();
        let updated_at = job.updated_at.to_rfc3339();
        let dismissed_at = job.dismissed_at.map(|d| d.to_rfc3339());
        let res = sqlx::query(
            "UPDATE transfer_jobs SET status = ?, phase = ?, transferred_bytes = ?, total_bytes = ?, speed_bytes_per_sec = ?, eta_seconds = ?, checksum = ?, error_message = ?, dismissed_at = ?, updated_at = ? WHERE id = ? AND status NOT IN ('cancelled','cancellation_requested')",
        )
        .bind(job.status.as_str())
        .bind(job.phase.as_str())
        .bind(job.transferred_bytes as i64)
        .bind(job.total_bytes as i64)
        .bind(job.speed_bytes_per_sec as i64)
        .bind(job.eta_seconds.map(|e| e as i64))
        .bind(&job.checksum)
        .bind(&job.error_message)
        .bind(&dismissed_at)
        .bind(&updated_at)
        .bind(&job.id)
        .execute(db)
        .await?;
        if res.rows_affected() == 0 {
            tracing::warn!("save conditional skipped: job {} already cancelled", job.id);
            return Ok(());
        }
        // Ensure row exists (insert if not found due to race on first save)
        let _ = sqlx::query(
            "INSERT OR IGNORE INTO transfer_jobs (id, user_id, name, transfer_type, source_connection_id, source_path, destination_connection_id, destination_path, status, phase, transferred_bytes, total_bytes, speed_bytes_per_sec, eta_seconds, checksum, error_message, dismissed_at, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&job.id)
        .bind(&job.user_id)
        .bind(&job.name)
        .bind(job.transfer_type.as_str())
        .bind(&job.source_connection_id)
        .bind(&job.source_path)
        .bind(&job.destination_connection_id)
        .bind(&job.destination_path)
        .bind(job.status.as_str())
        .bind(job.phase.as_str())
        .bind(job.transferred_bytes as i64)
        .bind(job.total_bytes as i64)
        .bind(job.speed_bytes_per_sec as i64)
        .bind(job.eta_seconds.map(|e| e as i64))
        .bind(&job.checksum)
        .bind(&job.error_message)
        .bind(&dismissed_at)
        .bind(&created_at)
        .bind(&updated_at)
        .execute(db)
        .await?;
        Ok(())
    }

    async fn save_job_conditional(
        db: &DbPool,
        job: &TransferJob,
        _allowed_prev: &[&str],
    ) -> anyhow::Result<()> {
        // Generic conditional save used for cancel path
        let _ = Self::save_job_to_db(db, job).await;
        Ok(())
    }

    /// Save or update a transfer job in SQLite (supports new execution_mode/staging columns with fallback)
    async fn save_job_to_db(db: &DbPool, job: &TransferJob) -> anyhow::Result<()> {
        let created_at = job.created_at.to_rfc3339();
        let updated_at = job.updated_at.to_rfc3339();
        let dismissed_at = job.dismissed_at.map(|d| d.to_rfc3339());

        // Try new schema first; fallback to old schema if migration not yet applied (e.g. in-memory tests)
        let res = sqlx::query(
            "INSERT INTO transfer_jobs (
                id, user_id, name, transfer_type, source_connection_id, source_path,
                destination_connection_id, destination_path, status, phase,
                execution_mode, staging,
                transferred_bytes, total_bytes, speed_bytes_per_sec,
                eta_seconds, checksum, error_message, dismissed_at, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                status = excluded.status,
                phase = excluded.phase,
                execution_mode = excluded.execution_mode,
                staging = excluded.staging,
                transferred_bytes = excluded.transferred_bytes,
                total_bytes = excluded.total_bytes,
                speed_bytes_per_sec = excluded.speed_bytes_per_sec,
                eta_seconds = excluded.eta_seconds,
                checksum = excluded.checksum,
                error_message = excluded.error_message,
                dismissed_at = excluded.dismissed_at,
                updated_at = excluded.updated_at",
        )
        .bind(&job.id)
        .bind(&job.user_id)
        .bind(&job.name)
        .bind(job.transfer_type.as_str())
        .bind(&job.source_connection_id)
        .bind(&job.source_path)
        .bind(&job.destination_connection_id)
        .bind(&job.destination_path)
        .bind(job.status.as_str())
        .bind(job.phase.as_str())
        .bind(job.execution_mode.as_str())
        .bind(job.staging.as_str())
        .bind(job.transferred_bytes as i64)
        .bind(job.total_bytes as i64)
        .bind(job.speed_bytes_per_sec as i64)
        .bind(job.eta_seconds.map(|e| e as i64))
        .bind(&job.checksum)
        .bind(&job.error_message)
        .bind(&dismissed_at)
        .bind(&created_at)
        .bind(&updated_at)
        .execute(db)
        .await;

        if let Err(e) = res {
            let msg = e.to_string();
            if msg.contains("no column") || msg.contains("has no column") {
                sqlx::query(
                    "INSERT INTO transfer_jobs (
                        id, user_id, name, transfer_type, source_connection_id, source_path,
                        destination_connection_id, destination_path, status, phase,
                        transferred_bytes, total_bytes, speed_bytes_per_sec,
                        eta_seconds, checksum, error_message, dismissed_at, created_at, updated_at
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    ON CONFLICT(id) DO UPDATE SET
                        status = excluded.status,
                        phase = excluded.phase,
                        transferred_bytes = excluded.transferred_bytes,
                        total_bytes = excluded.total_bytes,
                        speed_bytes_per_sec = excluded.speed_bytes_per_sec,
                        eta_seconds = excluded.eta_seconds,
                        checksum = excluded.checksum,
                        error_message = excluded.error_message,
                        dismissed_at = excluded.dismissed_at,
                        updated_at = excluded.updated_at",
                )
                .bind(&job.id)
                .bind(&job.user_id)
                .bind(&job.name)
                .bind(job.transfer_type.as_str())
                .bind(&job.source_connection_id)
                .bind(&job.source_path)
                .bind(&job.destination_connection_id)
                .bind(&job.destination_path)
                .bind(job.status.as_str())
                .bind(job.phase.as_str())
                .bind(job.transferred_bytes as i64)
                .bind(job.total_bytes as i64)
                .bind(job.speed_bytes_per_sec as i64)
                .bind(job.eta_seconds.map(|e| e as i64))
                .bind(&job.checksum)
                .bind(&job.error_message)
                .bind(&dismissed_at)
                .bind(&created_at)
                .bind(&updated_at)
                .execute(db)
                .await?;
            } else {
                return Err(e.into());
            }
        }

        Ok(())
    }
}
