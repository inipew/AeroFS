use super::planner::{TransferPlanner, TransferStrategy};
use crate::db::DbPool;
use crate::domain::VfsPath;
pub use crate::events::EventEnvelope;
use crate::events::{DomainEvent, EventJournal, ReplayOutcome};
use crate::runtime::{ResourceBudget, ResourceClass};
pub use crate::transfer::model::{
    CancelTransferError, RetryTransferError, TransferExecutionMode, TransferJob,
    TransferJobResponse, TransferPhase, TransferStaging, TransferStatus, TransferType,
};
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

#[derive(Clone, Copy, Debug)]
pub struct PersistenceCheckpoint {
    pub last_persisted_at: Instant,
    pub last_persisted_bytes: u64,
    pub last_persisted_phase: Option<TransferPhase>,
    pub last_persisted_status: Option<TransferStatus>,
}

#[derive(Clone)]
pub struct TransferManager {
    providers: Arc<RwLock<HashMap<String, Arc<dyn FileSystem>>>>,
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
    checkpoints: Arc<std::sync::Mutex<HashMap<String, PersistenceCheckpoint>>>,
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
        resource_budget: Arc<ResourceBudget>,
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
        let worker_semaphore = resource_budget.transfer_semaphore();
        let is_accepting_jobs = Arc::new(std::sync::atomic::AtomicBool::new(true));

        let jobs_clone = Arc::clone(&jobs);
        let cancel_tokens_clone = Arc::clone(&cancel_tokens);
        let event_journal_clone = Arc::clone(&event_journal);
        let db_clone = db.clone();
        let queue_tx_clone = queue_tx.clone();
        let completion_tx_clone = completion_tx.clone();
        let providers_clone = Arc::clone(&providers);
        let resource_budget_clone = Arc::clone(&resource_budget);

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
                let job_id = tokio::select! {
                    _ = scheduler_token.cancelled() => {
                        tracing::info!("transfer.scheduler.stop: shutdown requested");
                        is_accepting_clone.store(false, std::sync::atomic::Ordering::Release);
                        break;
                    }
                    maybe_id = rx.recv() => {
                        match maybe_id {
                            Some(id) => id,
                            None => break,
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
                            Err(_) => break,
                        }
                    }
                };

                let jobs_worker = Arc::clone(&jobs_clone);
                let cancel_tokens_worker = Arc::clone(&cancel_tokens_clone);
                let providers_worker = Arc::clone(&providers_clone);
                let event_journal_worker = Arc::clone(&event_journal_clone);
                let resource_budget_worker = Arc::clone(&resource_budget_clone);
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
                                let _ = event_journal_worker
                                    .append(DomainEvent::transfer_cancelled(&job), Some(&job.id))
                                    .await;
                            }
                        } else {
                            let _ = Self::save_job_to_db(&db_worker, &job).await;
                            let _ = event_journal_worker
                                .append(DomainEvent::transfer_progress(&job), Some(&job.id))
                                .await;

                            let result = Self::execute_job_with_retry(
                                &mut job,
                                &cancel_token,
                                &providers_worker,
                                &jobs_worker,
                                &event_journal_worker,
                                &db_worker,
                                &retries_task,
                                &resource_budget_worker,
                            )
                            .await;

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
                                let _ = Self::save_job_conditional(
                                    &db_worker,
                                    &job,
                                    &["cancelled", "cancellation_requested", "running", "queued"],
                                )
                                .await;
                                let _ = event_journal_worker
                                    .append(DomainEvent::transfer_cancelled(&job), Some(&job.id))
                                    .await;
                            } else {
                                match result {
                                    Ok(()) => {
                                        let still_cancelled = {
                                            let map = jobs_worker.read().await;
                                            map.get(&job.id)
                                                .map(|j| {
                                                    j.status == TransferStatus::Cancelled
                                                        || j.status == TransferStatus::CancellationRequested
                                                })
                                                .unwrap_or(false)
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
                                            let _ = event_journal_worker
                                                .append(DomainEvent::transfer_cancelled(&job), Some(&job.id))
                                                .await;
                                        } else {
                                            let _ = crate::transfer::checkpoint::TransferCheckpoint::delete(
                                                &db_worker,
                                                &job.id,
                                            )
                                            .await;
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
                                            let _ = event_journal_worker
                                                .append(
                                                    DomainEvent::file_change(
                                                        &job.destination_connection_id,
                                                        &job.destination_path,
                                                        "create",
                                                    ),
                                                    Some(&job.id),
                                                )
                                                .await;
                                            if job.transfer_type == TransferType::Move {
                                                let _ = event_journal_worker
                                                    .append(
                                                        DomainEvent::file_change(
                                                            &job.source_connection_id,
                                                            &job.source_path,
                                                            "delete",
                                                        ),
                                                        Some(&job.id),
                                                    )
                                                    .await;
                                            }
                                            let _ = event_journal_worker
                                                .append(DomainEvent::transfer_completed(&job), Some(&job.id))
                                                .await;
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
                                        let _ = event_journal_worker
                                            .append(DomainEvent::transfer_failed(&job), Some(&job.id))
                                            .await;
                                        let _ = completion_tx_worker.send((job.id.clone(), false));
                                    }
                                }
                            }
                        }
                    }

                    cancel_tokens_worker.write().await.remove(&job_id);
                });
            }
        });

        let checkpoints = Arc::new(std::sync::Mutex::new(HashMap::new()));

        Self {
            providers,
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
            checkpoints,
        }
    }

    pub fn completion_receiver(&self) -> broadcast::Receiver<(String, bool)> {
        self.completion_tx.subscribe()
    }

    pub(crate) fn cancel_token(&self, job_id: &str) -> Option<CancellationToken> {
        self.cancel_tokens.try_read().ok()?.get(job_id).cloned()
    }

    pub async fn try_enter_finalizing(
        &self,
        job_id: &str,
    ) -> Result<bool, crate::errors::AppError> {
        use crate::transfer::{TransferPhase, TransferStatus};
        let token_cancelled_early = self
            .cancel_tokens
            .try_read()
            .ok()
            .and_then(|m| m.get(job_id).map(|t| t.is_cancelled()))
            .unwrap_or(false);
        let mut map = self.jobs.write().await;
        let job = map.get_mut(job_id).ok_or_else(|| {
            crate::errors::AppError::NotFound(format!("Transfer job '{}' not found", job_id))
        })?;
        let token_cancelled = token_cancelled_early
            || self
                .cancel_tokens
                .try_read()
                .ok()
                .and_then(|m| m.get(job_id).map(|t| t.is_cancelled()))
                .unwrap_or(false);
        if token_cancelled
            || job.status == TransferStatus::Cancelled
            || job.status == TransferStatus::CancellationRequested
        {
            return Ok(false);
        }
        if matches!(
            job.phase,
            TransferPhase::Finalizing | TransferPhase::Verifying | TransferPhase::Completed
        ) {
            return Ok(false);
        }
        if job.status != TransferStatus::Running {
            return Ok(false);
        }
        job.phase = TransferPhase::Finalizing;
        job.updated_at = chrono::Utc::now();
        let job_clone = job.clone();
        drop(map);
        let _ = Self::save_job_to_db(&self.db, &job_clone).await;
        let _ = self
            .event_journal
            .append(
                crate::events::DomainEvent::transfer_progress(&job_clone),
                Some(job_id),
            )
            .await;
        Ok(true)
    }

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
            execution_mode: TransferExecutionMode::Background,
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

        Self::save_job_to_db(&self.db, &job)
            .await
            .map_err(|e| format!("Database persistence error: {}", e))?;
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
        let _ = self
            .event_journal
            .append(DomainEvent::transfer_progress(&job), Some(&job.id))
            .await;
        let _ = Self::save_job_to_db(&self.db, &job).await;
        job
    }

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

    pub async fn update_inline_progress(
        &self,
        job_id: &str,
        transferred: u64,
        total: u64,
        speed: u64,
        eta: Option<u64>,
    ) {
        let job = {
            let mut map = self.jobs.write().await;
            if let Some(j) = map.get_mut(job_id) {
                j.transferred_bytes = transferred;
                j.total_bytes = total;
                j.speed_bytes_per_sec = speed;
                j.eta_seconds = eta;
                j.updated_at = Utc::now();
                Some(j.clone())
            } else {
                None
            }
        };

        if let Some(job) = job {
            let _ = self
                .event_journal
                .append(DomainEvent::transfer_progress(&job), Some(&job.id))
                .await;

            let should_persist = {
                let mut cp_map = self.checkpoints.lock().unwrap();
                let now = Instant::now();
                let cp = cp_map
                    .entry(job_id.to_string())
                    .or_insert(PersistenceCheckpoint {
                        last_persisted_at: now,
                        last_persisted_bytes: 0,
                        last_persisted_phase: Some(job.phase),
                        last_persisted_status: Some(job.status),
                    });
                let time_elapsed = now.duration_since(cp.last_persisted_at).as_secs_f64() >= 1.0;
                let bytes_elapsed =
                    transferred.saturating_sub(cp.last_persisted_bytes) >= 4 * 1024 * 1024;
                let phase_changed = cp.last_persisted_phase != Some(job.phase);
                let status_changed = cp.last_persisted_status != Some(job.status);
                let is_terminal = job.status.is_terminal();
                let is_done = transferred >= total && total > 0;
                if time_elapsed
                    || bytes_elapsed
                    || phase_changed
                    || status_changed
                    || is_terminal
                    || is_done
                {
                    cp.last_persisted_at = now;
                    cp.last_persisted_bytes = transferred;
                    cp.last_persisted_phase = Some(job.phase);
                    cp.last_persisted_status = Some(job.status);
                    true
                } else {
                    false
                }
            };

            if should_persist {
                let _ = Self::save_job_to_db(&self.db, &job).await;
            }
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
            } else {
                None
            }
        };
        if let Some(job) = job_opt {
            let _ = Self::save_job_conditional_completed(&self.db, &job).await;
            let _ = self
                .event_journal
                .append(
                    DomainEvent::file_change(
                        &job.destination_connection_id,
                        &job.destination_path,
                        "upload",
                    ),
                    Some(&job.id),
                )
                .await;
            let _ = self
                .event_journal
                .append(DomainEvent::transfer_completed(&job), Some(&job.id))
                .await;
            let _ = self.completion_tx.send((job.id.clone(), true));
        }
        self.checkpoints.lock().unwrap().remove(job_id);
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
            } else {
                None
            }
        };
        if let Some(job) = job_opt {
            let _ = Self::save_job_to_db(&self.db, &job).await;
            let _ = self
                .event_journal
                .append(DomainEvent::transfer_failed(&job), Some(&job.id))
                .await;
            let _ = self.completion_tx.send((job.id.clone(), false));
        }
        self.checkpoints.lock().unwrap().remove(job_id);
        self.cancel_tokens.write().await.remove(job_id);
    }

    pub async fn cancel_inline_job(&self, job_id: &str) {
        let job_opt = {
            let mut map = self.jobs.write().await;
            if let Some(job) = map.get_mut(job_id) {
                if matches!(
                    job.phase,
                    TransferPhase::Finalizing | TransferPhase::Verifying | TransferPhase::Completed
                ) {
                    None
                } else {
                    job.status = TransferStatus::Cancelled;
                    job.speed_bytes_per_sec = 0;
                    job.eta_seconds = None;
                    job.updated_at = Utc::now();
                    Some(job.clone())
                }
            } else {
                None
            }
        };
        if let Some(job) = job_opt {
            let _ = Self::save_job_to_db(&self.db, &job).await;
            let _ = self
                .event_journal
                .append(DomainEvent::transfer_cancelled(&job), Some(&job.id))
                .await;
            let _ = self.completion_tx.send((job.id.clone(), false));
        }
        self.checkpoints.lock().unwrap().remove(job_id);
        self.cancel_tokens.write().await.remove(job_id);
    }

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

    pub async fn get_job(&self, id: &str) -> Option<TransferJob> {
        if let Some(j) = self.jobs.read().await.get(id).cloned() {
            return Some(j);
        }
        Self::load_single_job_from_db(&self.db, id)
            .await
            .ok()
            .flatten()
    }

    pub async fn insert_job_for_test(&self, job: TransferJob) {
        let _ = Self::save_job_to_db(&self.db, &job).await;
        self.jobs.write().await.insert(job.id.clone(), job);
    }

    pub async fn cancel_job(
        &self,
        id: &str,
        user_id: Option<&str>,
        is_admin: bool,
    ) -> Result<bool, CancelTransferError> {
        let (token_opt, is_queued, updated_job) = {
            let mut map = self.jobs.write().await;
            if let Some(job) = map.get_mut(id) {
                if !is_admin {
                    match (&job.user_id, user_id) {
                        (Some(owner), Some(uid)) if owner == uid => {}
                        _ => return Err(CancelTransferError::Unauthorized),
                    }
                }

                if job.status == TransferStatus::CancellationRequested {
                    let token = self.cancel_tokens.read().await.get(id).cloned();
                    (token, false, None)
                } else if !job.can_cancel() {
                    return Err(CancelTransferError::NotCancellable(id.to_string()));
                } else {
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
                    } else {
                        return Err(CancelTransferError::NotCancellable(id.to_string()));
                    }
                }
            } else {
                (None, false, None)
            }
        };

        if let Some(ref token) = token_opt {
            token.cancel();
        }
        let _ = crate::transfer::checkpoint::TransferCheckpoint::delete(&self.db, id).await;
        self.checkpoints.lock().unwrap().remove(id);

        if let Some(job) = updated_job {
            let _ = Self::save_job_to_db(&self.db, &job).await;
            if is_queued {
                let _ = self
                    .event_journal
                    .append(DomainEvent::transfer_cancelled(&job), Some(&job.id))
                    .await;
            } else {
                let _ = self
                    .event_journal
                    .append(DomainEvent::transfer_progress(&job), Some(&job.id))
                    .await;
            }
            Ok(true)
        } else if token_opt.is_some() {
            Ok(true)
        } else {
            if let Ok(Some(row)) =
                sqlx::query("SELECT user_id, status, phase FROM transfer_jobs WHERE id = ?")
                    .bind(id)
                    .fetch_optional(&self.db)
                    .await
            {
                let db_user_id: Option<String> = row.try_get("user_id").ok();
                let db_status_str: String = row.try_get("status").unwrap_or_default();
                let db_phase_str: String = row.try_get("phase").unwrap_or_default();
                let db_status = TransferStatus::from_str(&db_status_str);
                let db_phase = TransferPhase::from_str(&db_phase_str);

                if !is_admin {
                    match (&db_user_id, user_id) {
                        (Some(owner), Some(uid)) if owner == uid => {}
                        _ => return Err(CancelTransferError::Unauthorized),
                    }
                }

                if db_status == TransferStatus::CancellationRequested {
                    return Ok(true);
                }

                if !crate::transfer::model::is_cancellable_state(db_status, db_phase) {
                    return Err(CancelTransferError::NotCancellable(id.to_string()));
                }

                let now_str = Utc::now().to_rfc3339();
                let res = sqlx::query(
                    "UPDATE transfer_jobs SET status = 'cancelled', updated_at = ? WHERE id = ?",
                )
                .bind(&now_str)
                .bind(id)
                .execute(&self.db)
                .await;
                match res {
                    Ok(r) if r.rows_affected() > 0 => Ok(true),
                    Ok(_) => Err(CancelTransferError::NotFound(id.to_string())),
                    Err(e) => Err(CancelTransferError::Internal(e.to_string())),
                }
            } else {
                Err(CancelTransferError::NotFound(id.to_string()))
            }
        }
    }

    pub async fn retry_job(
        &self,
        id: &str,
        user_id: Option<&str>,
        is_admin: bool,
    ) -> Result<bool, RetryTransferError> {
        let job = {
            let map = self.jobs.read().await;
            map.get(id).cloned()
        };
        let job = match job {
            Some(j) => j,
            None => {
                if let Ok(Some(db_job)) = Self::load_single_job_from_db(&self.db, id).await {
                    db_job
                } else {
                    return Err(RetryTransferError::NotFound(id.to_string()));
                }
            }
        };

        if !is_admin {
            match (&job.user_id, user_id) {
                (Some(owner), Some(uid)) if owner == uid => {}
                _ => return Err(RetryTransferError::Unauthorized),
            }
        }

        if job.dismissed_at.is_some() {
            return Err(RetryTransferError::Dismissed(id.to_string()));
        }

        if !job.is_structurally_retryable() {
            if matches!(
                (job.transfer_type, job.execution_mode),
                (TransferType::Upload, TransferExecutionMode::Inline)
            ) {
                return Err(RetryTransferError::SourceUnavailable(format!(
                    "Transfer job '{}' is an inline browser upload and cannot be retried from server",
                    id
                )));
            }
            return Err(RetryTransferError::InvalidStatus(
                id.to_string(),
                format!(
                    "job in status '{:?}' cannot be retried (only Failed or Interrupted transfers can be retried)",
                    job.status
                ),
            ));
        }

        let (src_fs, _dst_fs) = {
            let providers_lock = self.providers.read().await;
            let src = providers_lock
                .get(&job.source_connection_id)
                .cloned()
                .ok_or_else(|| {
                    RetryTransferError::ProviderUnavailable(format!(
                        "Source connection '{}' not found",
                        job.source_connection_id
                    ))
                })?;
            let dst = providers_lock
                .get(&job.destination_connection_id)
                .cloned()
                .ok_or_else(|| {
                    RetryTransferError::ProviderUnavailable(format!(
                        "Destination connection '{}' not found",
                        job.destination_connection_id
                    ))
                })?;
            (src, dst)
        };

        let src_vfs = crate::domain::VfsPath::new(&job.source_connection_id, &job.source_path)
            .map_err(|e| {
                RetryTransferError::SourceUnavailable(format!("Invalid source path: {}", e))
            })?;

        if let Err(e) = src_fs.stat(&src_vfs).await {
            return Err(RetryTransferError::SourceUnavailable(format!(
                "Source path '{}' is not accessible: {}",
                job.source_path, e
            )));
        }

        let old_status = job.status;
        let old_phase = job.phase;
        let old_error = job.error_message.clone();

        let retry_phase =
            if job.transfer_type == TransferType::Move && old_phase == TransferPhase::CleaningUp {
                TransferPhase::CleaningUp
            } else {
                TransferPhase::Preparing
            };

        let updated_job = {
            let mut map = self.jobs.write().await;
            let j = map.entry(id.to_string()).or_insert_with(|| job.clone());
            if j.status != old_status || j.phase != old_phase {
                return Err(RetryTransferError::InvalidStatus(
                    id.to_string(),
                    "transfer state changed while retry was being validated".into(),
                ));
            }
            j.status = TransferStatus::Queued;
            j.phase = retry_phase;
            j.error_message = None;
            j.updated_at = Utc::now();
            j.clone()
        };

        if let Err(e) = Self::save_job_to_db(&self.db, &updated_job).await {
            let mut map = self.jobs.write().await;
            if let Some(j) = map.get_mut(id) {
                j.status = old_status;
                j.phase = old_phase;
                j.error_message = old_error;
                j.updated_at = Utc::now();
            }
            return Err(RetryTransferError::Internal(format!(
                "Failed to persist retry state: {}",
                e
            )));
        }

        let _ = self
            .event_journal
            .append(
                DomainEvent::transfer_progress(&updated_job),
                Some(&updated_job.id),
            )
            .await;

        if let Err(e) = self.queue_tx.send(id.to_string()).await {
            let rollback_job = {
                let mut map = self.jobs.write().await;
                let j = map.get_mut(id).unwrap();
                j.status = old_status;
                j.phase = old_phase;
                j.error_message = old_error;
                j.updated_at = Utc::now();
                j.clone()
            };
            let _ = Self::save_job_to_db(&self.db, &rollback_job).await;
            let _ = self
                .event_journal
                .append(
                    DomainEvent::transfer_progress(&rollback_job),
                    Some(&rollback_job.id),
                )
                .await;
            return Err(RetryTransferError::Internal(format!(
                "Failed to enqueue retry job: {}",
                e
            )));
        }

        Ok(true)
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
                map.remove(id);
                Some(j)
            } else {
                None
            }
        };

        if let Some(job) = updated_job {
            let _ = Self::save_job_to_db(&self.db, &job).await;
            Ok(true)
        } else {
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

            for id in ids_to_evict {
                map.remove(&id);
            }
        }

        for j in &jobs_to_persist {
            let _ = Self::save_job_to_db(&self.db, j).await;
        }

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

    #[allow(clippy::too_many_arguments)]
    async fn execute_job_with_retry(
        job: &mut TransferJob,
        cancel_token: &CancellationToken,
        providers: &Arc<RwLock<HashMap<String, Arc<dyn FileSystem>>>>,
        jobs_map: &Arc<RwLock<HashMap<String, TransferJob>>>,
        event_journal: &Arc<EventJournal>,
        db: &DbPool,
        max_retries: &Arc<AtomicUsize>,
        resource_budget: &Arc<ResourceBudget>,
    ) -> anyhow::Result<()> {
        let mut attempt = 0;

        loop {
            if cancel_token.is_cancelled() {
                return Err(anyhow::anyhow!("Transfer cancelled by user"));
            }

            attempt += 1;
            let max_attempts = max_retries.load(Ordering::Relaxed).max(1);

            let resource_class = {
                let providers = providers.read().await;
                let source_local = providers
                    .get(&job.source_connection_id)
                    .map(|provider| provider.is_local())
                    .unwrap_or(false);
                let destination_local = providers
                    .get(&job.destination_connection_id)
                    .map(|provider| provider.is_local())
                    .unwrap_or(false);
                match (source_local, destination_local) {
                    (true, true) => ResourceClass::LocalIo,
                    (false, false) => ResourceClass::NetworkIo,
                    _ => ResourceClass::MixedIo,
                }
            };
            let _resource_permit = resource_budget
                .acquire(resource_class)
                .await
                .map_err(|_| anyhow::anyhow!("Transfer resource budget closed"))?;

            match Self::execute_job(job, cancel_token, providers, jobs_map, event_journal, db).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    drop(_resource_permit);
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

        if job.transfer_type == TransferType::Move && job.phase == TransferPhase::CleaningUp {
            src_fs
                .delete(&src_vfs)
                .await
                .map_err(|e| anyhow::anyhow!("Delete source failed during Move cleanup: {}", e))?;
            job.phase = TransferPhase::Completed;
            job.transferred_bytes = job.total_bytes;
            job.speed_bytes_per_sec = 0;
            job.eta_seconds = Some(0);
            job.updated_at = Utc::now();
            return Ok(());
        }

        job.phase = TransferPhase::Preparing;
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

            if let Err(e) = dst_fs.create_dir(&dst_vfs).await {
                if !dst_fs
                    .stat(&dst_vfs)
                    .await
                    .map(|m| m.kind == crate::domain::FileKind::Directory)
                    .unwrap_or(false)
                {
                    return Err(anyhow::anyhow!(
                        "Failed creating root destination directory '{}': {}",
                        dst_vfs.path,
                        e
                    ));
                }
            }

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

            let concurrency = if dst_fs.capabilities().write_can_multi { 8 } else { 4 };
            let (file_tx, file_rx) = tokio::sync::mpsc::channel::<ItemToTransfer>(128);

            let transferred_atomic = Arc::new(AtomicU64::new(0));
            let total_bytes_atomic = Arc::new(AtomicU64::new(0));

            let worker_src_fs = Arc::clone(&src_fs);
            let worker_dst_fs = Arc::clone(&dst_fs);
            let worker_src_vfs = src_vfs.clone();
            let worker_dst_vfs = dst_vfs.clone();
            let worker_src_conn = job.source_connection_id.clone();
            let worker_dst_conn = job.destination_connection_id.clone();
            let worker_is_move = job.transfer_type == TransferType::Move;
            let worker_cancel_token = cancel_token.clone();
            let worker_transferred = Arc::clone(&transferred_atomic);

            let ticker_cancel = cancel_token.clone();
            let ticker_transferred = Arc::clone(&transferred_atomic);
            let ticker_total = Arc::clone(&total_bytes_atomic);
            let ticker_job_id = job.id.clone();
            let ticker_jobs_map = Arc::clone(jobs_map);
            let ticker_journal = Arc::clone(event_journal);

            let ticker_handle = tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_millis(250));
                let mut rate_estimator =
                    crate::transfer::rate::TransferRateEstimator::new(Instant::now(), 0);
                loop {
                    tokio::select! {
                        _ = ticker_cancel.cancelled() => break,
                        _ = interval.tick() => {
                            let now = Instant::now();
                            let current_bytes = ticker_transferred.load(Ordering::Relaxed);
                            let total = ticker_total.load(Ordering::Relaxed);
                            let sample = rate_estimator.observe(now, current_bytes, total);
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
                                        j.speed_bytes_per_sec = sample.speed_bytes_per_sec;
                                        j.eta_seconds = sample.eta_seconds;
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

            drop(file_tx);

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

            ticker_handle.abort();

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

        let total_bytes = job.total_bytes;

        let target_perms = crate::domain::resolve_destination_permissions_strict(
            &dst_fs,
            &dst_vfs,
            false,
            crate::domain::PermissionInheritanceMode::InheritExistingOrParent,
        )
        .await
        .map_err(|error| {
            anyhow::anyhow!(
                "Destination permission lookup failed for '{}': {}",
                dst_vfs.path,
                error
            )
        })?;

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

        let can_append = dst_fs.capabilities().write_can_append;
        let can_range_read = src_fs.capabilities().range_read;
        let src_meta = src_fs.stat(&src_vfs).await.ok();

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
                let _ = dst_fs.delete(&write_target_vfs).await;
                let _ = crate::transfer::checkpoint::TransferCheckpoint::delete(db, &job.id).await;
                0
            }
        } else {
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

        let (mut pipe_writer, pipe_reader) = tokio::io::duplex(64 * 1024);

        let job_id = job.id.clone();
        let jobs_map_clone = Arc::clone(jobs_map);
        let event_journal_clone = Arc::clone(event_journal);
        let db_clone = db.clone();
        let cancel_token_pump = cancel_token.clone();
        let staging_path_clone = staging_path.clone();
        let source_etag_clone = src_meta.map(|m| m.etag);

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

        let is_clean_start = resume_offset == 0;
        let pump_handle = tokio::spawn(async move {
            let mut buffer = vec![0u8; 64 * 1024];
            let mut transferred = resume_offset;
            let mut hasher = Sha256::new();
            let mut rate_estimator =
                crate::transfer::rate::TransferRateEstimator::new(Instant::now(), 0);
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

                tokio::select! {
                    _ = cancel_token_pump.cancelled() => {
                        return Err(anyhow::anyhow!("Transfer cancelled by user"));
                    }
                    res = pipe_writer.write_all(&buffer[..n]) => res?,
                };

                transferred += n as u64;
                let bytes_since_start = transferred.saturating_sub(resume_offset);
                let now = Instant::now();

                if now.duration_since(last_emit) >= Duration::from_millis(250)
                    || transferred == total_bytes
                {
                    last_emit = now;
                    let sample = rate_estimator.observe(
                        now,
                        bytes_since_start,
                        total_bytes.saturating_sub(resume_offset),
                    );
                    let speed = sample.speed_bytes_per_sec;
                    let eta = sample.eta_seconds;

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
            drop(pipe_writer);

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

        if use_staging {
            if let Some(perms) = target_perms.as_deref() {
                if let Err(error) = dst_fs.set_permissions(&write_target_vfs, perms).await {
                    let _ = dst_fs.delete(&write_target_vfs).await;
                    return Err(anyhow::anyhow!(
                        "Failed to apply inherited permissions '{}' to staging file '{}': {}",
                        perms,
                        write_target_vfs.path,
                        error
                    ));
                }
            }

            if let Err(e) = dst_fs.rename(&write_target_vfs, &dst_vfs).await {
                let _ = dst_fs.delete(&write_target_vfs).await;
                return Err(anyhow::anyhow!(
                    "Failed to promote staging file to final destination '{}': {}",
                    dst_vfs.path,
                    e
                ));
            }
        } else if let Some(perms) = target_perms.as_deref() {
            if let Err(error) = dst_fs.set_permissions(&dst_vfs, perms).await {
                return Err(anyhow::anyhow!(
                    "Transfer content for '{}' was written, but applying inherited permissions '{}' failed: {}. Filesystem mutation committed; recovery required",
                    dst_vfs.path,
                    perms,
                    error
                ));
            }
        }

        job.transferred_bytes = transferred_bytes;
        job.checksum = checksum.clone();

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

    fn row_to_job(r: &sqlx::sqlite::SqliteRow) -> TransferJob {
        use sqlx::Row;
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

        let execution_mode: String = r
            .try_get("execution_mode")
            .unwrap_or_else(|_| "inline".to_string());
        let staging: String = r.try_get("staging").unwrap_or_else(|_| "none".to_string());
        TransferJob {
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
        }
    }

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

        Ok(rows.iter().map(Self::row_to_job).collect())
    }

    async fn load_single_job_from_db(db: &DbPool, id: &str) -> anyhow::Result<Option<TransferJob>> {
        let row = sqlx::query(
            "SELECT id, user_id, name, transfer_type, source_connection_id, source_path,
                    destination_connection_id, destination_path, status, phase,
                    transferred_bytes, total_bytes, speed_bytes_per_sec,
                    eta_seconds, checksum, error_message, dismissed_at, created_at, updated_at
             FROM transfer_jobs
             WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(db)
        .await?;

        Ok(row.as_ref().map(Self::row_to_job))
    }

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
        let _ = Self::save_job_to_db(db, job).await;
        Ok(())
    }

    async fn save_job_to_db(db: &DbPool, job: &TransferJob) -> anyhow::Result<()> {
        let created_at = job.created_at.to_rfc3339();
        let updated_at = job.updated_at.to_rfc3339();
        let dismissed_at = job.dismissed_at.map(|d| d.to_rfc3339());

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
