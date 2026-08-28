use crate::db::DbPool;
use crate::domain::VfsPath;
use crate::vfs::FileSystem;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TransferType {
    Copy,
    Move,
    Upload,
    Sync,
}

impl TransferType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TransferType::Copy => "copy",
            TransferType::Move => "move",
            TransferType::Upload => "upload",
            TransferType::Sync => "sync",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "move" => TransferType::Move,
            "upload" => TransferType::Upload,
            "sync" => TransferType::Sync,
            _ => TransferType::Copy,
        }
    }
}

impl std::str::FromStr for TransferType {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(TransferType::from_str(s))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TransferStatus {
    Queued,
    Running,
    CancellationRequested,
    Cancelled,
    Interrupted,
    Completed,
    Failed,
}

impl TransferStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TransferStatus::Queued => "queued",
            TransferStatus::Running => "running",
            TransferStatus::CancellationRequested => "cancellation_requested",
            TransferStatus::Cancelled => "cancelled",
            TransferStatus::Interrupted => "interrupted",
            TransferStatus::Completed => "completed",
            TransferStatus::Failed => "failed",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "running" => TransferStatus::Running,
            "cancellation_requested" => TransferStatus::CancellationRequested,
            "cancelled" => TransferStatus::Cancelled,
            "interrupted" => TransferStatus::Interrupted,
            "completed" => TransferStatus::Completed,
            "failed" => TransferStatus::Failed,
            _ => TransferStatus::Queued,
        }
    }
}

impl std::str::FromStr for TransferStatus {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(TransferStatus::from_str(s))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TransferPhase {
    Preparing,
    Transferring,
    Finalizing,
    Verifying,
    CleaningUp,
    Completed,
}

impl TransferPhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            TransferPhase::Preparing => "preparing",
            TransferPhase::Transferring => "transferring",
            TransferPhase::Finalizing => "finalizing",
            TransferPhase::Verifying => "verifying",
            TransferPhase::CleaningUp => "cleaning_up",
            TransferPhase::Completed => "completed",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "transferring" => TransferPhase::Transferring,
            "finalizing" => TransferPhase::Finalizing,
            "verifying" => TransferPhase::Verifying,
            "cleaning_up" => TransferPhase::CleaningUp,
            "completed" => TransferPhase::Completed,
            _ => TransferPhase::Preparing,
        }
    }
}

impl std::str::FromStr for TransferPhase {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(TransferPhase::from_str(s))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TransferJob {
    pub id: String,
    pub user_id: Option<String>,
    pub name: String,
    pub transfer_type: TransferType,
    pub source_connection_id: String,
    pub source_path: String,
    pub destination_connection_id: String,
    pub destination_path: String,
    pub status: TransferStatus,
    pub phase: TransferPhase,
    pub transferred_bytes: u64,
    pub total_bytes: u64,
    pub speed_bytes_per_sec: u64,
    pub eta_seconds: Option<u64>,
    pub checksum: Option<String>,
    pub error_message: Option<String>,
    pub dismissed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WsEvent {
    #[serde(rename = "transfer_progress")]
    TransferProgress(TransferJob),
    #[serde(rename = "transfer_completed")]
    TransferCompleted(TransferJob),
    #[serde(rename = "transfer_failed")]
    TransferFailed(TransferJob),
    #[serde(rename = "file_change")]
    FileChange {
        connection_id: String,
        path: String,
        action: String,
    },
    #[serde(rename = "resync_required")]
    ResyncRequired {
        reason: String,
        latest_sequence: u64,
    },
}

#[derive(Debug, Clone)]
pub enum ReplayResult {
    Events(Vec<EventEnvelope>),
    Expired { latest_sequence: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub id: String,
    pub sequence: u64,
    pub timestamp: DateTime<Utc>,
    #[serde(flatten)]
    pub event: WsEvent,
}

#[derive(Clone)]
pub struct TransferManager {
    jobs: Arc<RwLock<HashMap<String, TransferJob>>>,
    cancel_tokens: Arc<RwLock<HashMap<String, CancellationToken>>>,
    queue_tx: mpsc::Sender<String>,
    event_tx: broadcast::Sender<EventEnvelope>,
    sequence_counter: Arc<AtomicU64>,
    event_history: Arc<RwLock<VecDeque<EventEnvelope>>>,
    db: DbPool,
    max_concurrent_workers: Arc<AtomicUsize>,
    max_retry_attempts: Arc<AtomicUsize>,
    worker_notify: Arc<tokio::sync::Notify>,
}

impl TransferManager {
    pub fn new(
        providers: Arc<RwLock<HashMap<String, Arc<dyn FileSystem>>>>,
        db: DbPool,
        max_concurrent_workers: usize,
    ) -> Self {
        let (queue_tx, queue_rx) = mpsc::channel::<String>(200);
        let (event_tx, _) = broadcast::channel::<EventEnvelope>(400);

        let jobs: Arc<RwLock<HashMap<String, TransferJob>>> = Arc::new(RwLock::new(HashMap::new()));
        let cancel_tokens: Arc<RwLock<HashMap<String, CancellationToken>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let sequence_counter = Arc::new(AtomicU64::new(1));
        let event_history = Arc::new(RwLock::new(VecDeque::with_capacity(500)));
        let max_concurrent_workers_arc =
            Arc::new(AtomicUsize::new(max_concurrent_workers.clamp(1, 64)));
        let max_retry_attempts_arc = Arc::new(AtomicUsize::new(3));
        let worker_notify = Arc::new(tokio::sync::Notify::new());

        let jobs_clone = Arc::clone(&jobs);
        let cancel_tokens_clone = Arc::clone(&cancel_tokens);
        let event_tx_clone = event_tx.clone();
        let db_clone = db.clone();
        let queue_tx_clone = queue_tx.clone();
        let sequence_counter_clone = Arc::clone(&sequence_counter);
        let event_history_clone = Arc::clone(&event_history);

        // 1. Initial startup recovery: Load jobs from SQLite into memory (bound to non-dismissed / recent active)
        let db_init = db.clone();
        let jobs_init = Arc::clone(&jobs);
        tokio::spawn(async move {
            if let Ok(saved_jobs) = Self::load_jobs_from_db(&db_init).await {
                let mut map = jobs_init.write().await;
                for mut job in saved_jobs {
                    // Only keep non-dismissed and active/recent jobs in RAM
                    if job.dismissed_at.is_some() {
                        continue;
                    }
                    // Mark interrupted 'running' jobs as interrupted on restart
                    if job.status == TransferStatus::Running {
                        job.status = TransferStatus::Interrupted;
                        job.error_message = Some("Transfer interrupted by server restart".into());
                        let _ = Self::save_job_to_db(&db_init, &job).await;
                    } else if job.status == TransferStatus::Queued {
                        let _ = queue_tx_clone.send(job.id.clone()).await;
                    }
                    map.insert(job.id.clone(), job);
                }
            }
        });

        // 2. Multi-Worker Concurrent Transfer Scheduler (Dynamic bounded dispatcher)
        let queue_rx_shared = Arc::new(Mutex::new(queue_rx));
        let active_workers = Arc::new(AtomicUsize::new(0));
        let max_workers_clone = Arc::clone(&max_concurrent_workers_arc);
        let retries_clone = Arc::clone(&max_retry_attempts_arc);
        let worker_notify_clone = Arc::clone(&worker_notify);

        tokio::spawn(async move {
            let mut rx = queue_rx_shared.lock().await;
            while let Some(job_id) = rx.recv().await {
                // Wait until active_workers < max_concurrent
                while active_workers.load(Ordering::SeqCst)
                    >= max_workers_clone.load(Ordering::SeqCst)
                {
                    worker_notify_clone.notified().await;
                }

                active_workers.fetch_add(1, Ordering::SeqCst);

                let jobs_worker = Arc::clone(&jobs_clone);
                let cancel_tokens_worker = Arc::clone(&cancel_tokens_clone);
                let providers_worker = Arc::clone(&providers);
                let event_tx_worker = event_tx_clone.clone();
                let seq_worker = Arc::clone(&sequence_counter_clone);
                let hist_worker = Arc::clone(&event_history_clone);
                let db_worker = db_clone.clone();
                let active_workers_task = Arc::clone(&active_workers);
                let notify_task = Arc::clone(&worker_notify_clone);
                let retries_task = Arc::clone(&retries_clone);

                tokio::spawn(async move {
                    let job_opt = {
                        let map = jobs_worker.read().await;
                        map.get(&job_id).cloned()
                    };

                    let cancel_token = {
                        let mut tokens = cancel_tokens_worker.write().await;
                        tokens
                            .entry(job_id.clone())
                            .or_insert_with(CancellationToken::new)
                            .clone()
                    };

                    if let Some(mut job) = job_opt {
                        // Handle cancelled or pending-cancellation jobs
                        if job.status == TransferStatus::Cancelled {
                            // Already marked cancelled
                        } else if job.status == TransferStatus::CancellationRequested
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
                            let _ = Self::save_job_to_db(&db_worker, &job).await;
                            Self::send_enveloped_event(
                                &event_tx_worker,
                                &seq_worker,
                                &hist_worker,
                                WsEvent::TransferFailed(job),
                            );
                        } else {
                            job.status = TransferStatus::Running;
                            job.updated_at = Utc::now();
                            {
                                let mut map = jobs_worker.write().await;
                                map.insert(job.id.clone(), job.clone());
                            }
                            let _ = Self::save_job_to_db(&db_worker, &job).await;
                            Self::send_enveloped_event(
                                &event_tx_worker,
                                &seq_worker,
                                &hist_worker,
                                WsEvent::TransferProgress(job.clone()),
                            );

                            // Execute robust bounded-stream transfer with retry & instant CancellationToken abort
                            let result = Self::execute_job_with_retry(
                                &mut job,
                                &cancel_token,
                                &providers_worker,
                                &jobs_worker,
                                &event_tx_worker,
                                &seq_worker,
                                &hist_worker,
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
                                let _ = Self::save_job_to_db(&db_worker, &job).await;
                                Self::send_enveloped_event(
                                    &event_tx_worker,
                                    &seq_worker,
                                    &hist_worker,
                                    WsEvent::TransferFailed(job),
                                );
                            } else {
                                match result {
                                    Ok(()) => {
                                        job.status = TransferStatus::Completed;
                                        job.phase = TransferPhase::Completed;
                                        job.speed_bytes_per_sec = 0;
                                        job.eta_seconds = Some(0);
                                        job.updated_at = Utc::now();
                                        {
                                            let mut map = jobs_worker.write().await;
                                            map.insert(job.id.clone(), job.clone());
                                        }
                                        let _ = Self::save_job_to_db(&db_worker, &job).await;
                                        // 1. Broadcast real-time FileChange event FIRST so open panels auto-refresh immediately (Plan 41 #22)
                                        Self::send_enveloped_event(
                                            &event_tx_worker,
                                            &seq_worker,
                                            &hist_worker,
                                            WsEvent::FileChange {
                                                connection_id: job
                                                    .destination_connection_id
                                                    .clone(),
                                                path: job.destination_path.clone(),
                                                action: "create".into(),
                                            },
                                        );
                                        if job.transfer_type == TransferType::Move {
                                            Self::send_enveloped_event(
                                                &event_tx_worker,
                                                &seq_worker,
                                                &hist_worker,
                                                WsEvent::FileChange {
                                                    connection_id: job.source_connection_id.clone(),
                                                    path: job.source_path.clone(),
                                                    action: "delete".into(),
                                                },
                                            );
                                        }

                                        // 2. Then emit TransferCompleted
                                        Self::send_enveloped_event(
                                            &event_tx_worker,
                                            &seq_worker,
                                            &hist_worker,
                                            WsEvent::TransferCompleted(job),
                                        );
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
                                        Self::send_enveloped_event(
                                            &event_tx_worker,
                                            &seq_worker,
                                            &hist_worker,
                                            WsEvent::TransferFailed(job),
                                        );
                                    }
                                }
                            }
                        }
                    }

                    // Clean up cancellation token
                    cancel_tokens_worker.write().await.remove(&job_id);

                    active_workers_task.fetch_sub(1, Ordering::SeqCst);
                    notify_task.notify_waiters();
                });
            }
        });

        Self {
            jobs,
            cancel_tokens,
            queue_tx,
            event_tx,
            sequence_counter,
            event_history,
            db,
            max_concurrent_workers: max_concurrent_workers_arc,
            max_retry_attempts: max_retry_attempts_arc,
            worker_notify,
        }
    }

    /// Dynamically update transfer concurrency worker limit and max retry count without restart (P1 #16 & #17)
    pub fn update_limits(&self, max_concurrent: usize, max_retries: usize) {
        self.max_concurrent_workers
            .store(max_concurrent.clamp(1, 64), Ordering::SeqCst);
        self.max_retry_attempts
            .store(max_retries.clamp(1, 10), Ordering::SeqCst);
        self.worker_notify.notify_waiters();
    }

    pub fn set_max_concurrent_transfers(&self, max_concurrent: usize) {
        self.update_limits(
            max_concurrent,
            self.max_retry_attempts.load(Ordering::Relaxed),
        );
    }

    fn send_enveloped_event(
        event_tx: &broadcast::Sender<EventEnvelope>,
        seq_counter: &Arc<AtomicU64>,
        event_history: &Arc<RwLock<VecDeque<EventEnvelope>>>,
        event: WsEvent,
    ) {
        let envelope = EventEnvelope {
            id: Uuid::new_v4().to_string(),
            sequence: seq_counter.fetch_add(1, Ordering::SeqCst),
            timestamp: Utc::now(),
            event,
        };

        if let Ok(mut hist) = event_history.try_write() {
            if hist.len() >= 500 {
                hist.pop_front();
            }
            hist.push_back(envelope.clone());
        } else {
            let hist_clone = Arc::clone(event_history);
            let env_clone = envelope.clone();
            tokio::spawn(async move {
                let mut hist = hist_clone.write().await;
                if hist.len() >= 500 {
                    hist.pop_front();
                }
                hist.push_back(env_clone);
            });
        }

        let _ = event_tx.send(envelope);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EventEnvelope> {
        self.event_tx.subscribe()
    }

    pub async fn get_events_since(&self, since_seq: u64) -> ReplayResult {
        let hist = self.event_history.read().await;
        if hist.is_empty() {
            return ReplayResult::Events(Vec::new());
        }
        if since_seq == 0 {
            return ReplayResult::Events(hist.iter().cloned().collect());
        }
        let oldest_seq = hist.front().map(|f| f.sequence).unwrap_or(0);
        if since_seq + 1 < oldest_seq {
            return ReplayResult::Expired {
                latest_sequence: self.sequence_counter.load(Ordering::SeqCst),
            };
        }
        let events = hist
            .iter()
            .filter(|e| e.sequence > since_seq)
            .cloned()
            .collect();
        ReplayResult::Events(events)
    }

    pub fn broadcast_event(&self, event: WsEvent) {
        Self::send_enveloped_event(
            &self.event_tx,
            &self.sequence_counter,
            &self.event_history,
            event,
        );
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

        // 2. Insert into in-memory state and register CancellationToken
        {
            let mut map = self.jobs.write().await;
            map.insert(id.clone(), job.clone());
        }
        {
            let mut tokens = self.cancel_tokens.write().await;
            tokens.insert(id.clone(), CancellationToken::new());
        }

        Self::send_enveloped_event(
            &self.event_tx,
            &self.sequence_counter,
            &self.event_history,
            WsEvent::TransferProgress(job),
        );
        self.queue_tx
            .send(id.clone())
            .await
            .map_err(|e| format!("Failed to queue transfer job: {}", e))?;

        Ok(id)
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

        if let Some(job) = updated_job {
            let _ = Self::save_job_to_db(&self.db, &job).await;
            if is_queued {
                Self::send_enveloped_event(
                    &self.event_tx,
                    &self.sequence_counter,
                    &self.event_history,
                    WsEvent::TransferFailed(job),
                );
            } else {
                Self::send_enveloped_event(
                    &self.event_tx,
                    &self.sequence_counter,
                    &self.event_history,
                    WsEvent::TransferProgress(job),
                );
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
            let now = Utc::now().to_rfc3339();
            let res = if is_admin {
                sqlx::query(
                    "UPDATE transfer_jobs SET dismissed_at = ?, updated_at = ? WHERE id = ? AND dismissed_at IS NULL",
                )
                .bind(&now)
                .bind(&now)
                .bind(id)
                .execute(&self.db)
                .await
            } else if let Some(uid) = user_id {
                sqlx::query(
                    "UPDATE transfer_jobs SET dismissed_at = ?, updated_at = ? WHERE id = ? AND user_id = ? AND dismissed_at IS NULL",
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

        // Also update any finished jobs in DB that might have already been evicted from RAM
        if is_admin {
            let _ = sqlx::query(
                "UPDATE transfer_jobs SET dismissed_at = ?, updated_at = ? WHERE dismissed_at IS NULL AND status IN ('completed', 'failed', 'cancelled')",
            )
            .bind(&now_str)
            .bind(&now_str)
            .execute(&self.db)
            .await;
        } else if let Some(uid) = user_id {
            let _ = sqlx::query(
                "UPDATE transfer_jobs SET dismissed_at = ?, updated_at = ? WHERE dismissed_at IS NULL AND user_id = ? AND status IN ('completed', 'failed', 'cancelled')",
            )
            .bind(&now_str)
            .bind(&now_str)
            .bind(uid)
            .execute(&self.db)
            .await;
        }

        Ok(count)
    }

    /// Execute transfer with exponential backoff retry for transient network hiccups (Dynamic retries P1 #17)
    #[allow(clippy::too_many_arguments)]
    async fn execute_job_with_retry(
        job: &mut TransferJob,
        cancel_token: &CancellationToken,
        providers: &Arc<RwLock<HashMap<String, Arc<dyn FileSystem>>>>,
        jobs_map: &Arc<RwLock<HashMap<String, TransferJob>>>,
        event_tx: &broadcast::Sender<EventEnvelope>,
        seq_counter: &Arc<AtomicU64>,
        event_history: &Arc<RwLock<VecDeque<EventEnvelope>>>,
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
            match Self::execute_job(
                job,
                cancel_token,
                providers,
                jobs_map,
                event_tx,
                seq_counter,
                event_history,
                db,
            )
            .await
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

                    // Classify permanent errors (404, 403, 401, Invalid Path, Unsupported, Cancelled)
                    let err_msg = e.to_string().to_lowercase();
                    let is_permanent = err_msg.contains("cancelled")
                        || err_msg.contains("not found")
                        || err_msg.contains("permission denied")
                        || err_msg.contains("forbidden")
                        || err_msg.contains("unauthorized")
                        || err_msg.contains("invalid path")
                        || err_msg.contains("unsupported");

                    let retry_policy = crate::domain::RetryPolicy::new(max_attempts);
                    if is_permanent || attempt >= max_attempts {
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

                    job.phase = TransferPhase::Preparing;
                    job.transferred_bytes = 0;
                    job.speed_bytes_per_sec = 0;
                    job.eta_seconds = None;
                    job.updated_at = Utc::now();
                    {
                        let mut map = jobs_map.write().await;
                        if let Some(j) = map.get_mut(&job.id) {
                            j.phase = TransferPhase::Preparing;
                            j.transferred_bytes = 0;
                            j.speed_bytes_per_sec = 0;
                            j.eta_seconds = None;
                            j.updated_at = Utc::now();
                        }
                    }
                    Self::send_enveloped_event(
                        event_tx,
                        seq_counter,
                        event_history,
                        WsEvent::TransferProgress(job.clone()),
                    );

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
    async fn execute_job(
        job: &mut TransferJob,
        cancel_token: &CancellationToken,
        providers: &Arc<RwLock<HashMap<String, Arc<dyn FileSystem>>>>,
        jobs_map: &Arc<RwLock<HashMap<String, TransferJob>>>,
        event_tx: &broadcast::Sender<EventEnvelope>,
        seq_counter: &Arc<AtomicU64>,
        event_history: &Arc<RwLock<VecDeque<EventEnvelope>>>,
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
            Self::send_enveloped_event(
                event_tx,
                seq_counter,
                event_history,
                WsEvent::TransferProgress(job.clone()),
            );
        }

        // FAST-PATH: Native atomic rename for same-connection Move (preserves inode, permissions, timestamps, instant)
        if job.transfer_type == TransferType::Move
            && job.source_connection_id == job.destination_connection_id
        {
            match src_fs.rename(&src_vfs, &dst_vfs).await {
                Ok(_) => {
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

        if meta.kind == crate::domain::FileKind::Directory {
            const MAX_TRANSFER_DIR_ENTRIES: usize = 100_000;
            const MAX_TRANSFER_DIR_DEPTH: usize = 64;

            // Recursive directory transfer!
            #[derive(Debug)]
            struct ItemToTransfer {
                rel_path: String,
                is_dir: bool,
                size: u64,
            }

            async fn scan_dir_recursive(
                fs: &Arc<dyn FileSystem>,
                cancel_token: &CancellationToken,
                conn_id: &str,
                base_vfs: &VfsPath,
                current_rel: &str,
                depth: usize,
                items: &mut Vec<ItemToTransfer>,
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
                if items.len() >= MAX_TRANSFER_DIR_ENTRIES {
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

                let entries = fs
                    .list(&current_vfs)
                    .await
                    .map_err(|e| anyhow::anyhow!("List failed: {}", e))?;
                for entry in entries {
                    if cancel_token.is_cancelled() {
                        return Err(anyhow::anyhow!("Transfer cancelled by user"));
                    }
                    let child_rel = if current_rel.is_empty() {
                        entry.name.clone()
                    } else {
                        format!("{}/{}", current_rel, entry.name)
                    };

                    if entry.kind == crate::domain::FileKind::Directory {
                        items.push(ItemToTransfer {
                            rel_path: child_rel.clone(),
                            is_dir: true,
                            size: 0,
                        });
                        Box::pin(scan_dir_recursive(
                            fs,
                            cancel_token,
                            conn_id,
                            base_vfs,
                            &child_rel,
                            depth + 1,
                            items,
                        ))
                        .await?;
                    } else {
                        items.push(ItemToTransfer {
                            rel_path: child_rel,
                            is_dir: false,
                            size: entry.size.unwrap_or(0),
                        });
                    }
                }
                Ok(())
            }

            let mut items = Vec::new();
            scan_dir_recursive(
                &src_fs,
                cancel_token,
                &job.source_connection_id,
                &src_vfs,
                "",
                0,
                &mut items,
            )
            .await?;

            let total_bytes: u64 = items.iter().map(|i| i.size).sum();
            job.total_bytes = total_bytes;
            job.transferred_bytes = 0;
            job.phase = TransferPhase::Preparing;
            {
                let mut map = jobs_map.write().await;
                if let Some(j) = map.get_mut(&job.id) {
                    j.total_bytes = total_bytes;
                    j.phase = TransferPhase::Preparing;
                }
            }
            Self::send_enveloped_event(
                event_tx,
                seq_counter,
                event_history,
                WsEvent::TransferProgress(job.clone()),
            );

            // 1. Create root destination directory
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
            if let Some(perms) = crate::domain::resolve_destination_permissions(
                &dst_fs,
                &dst_vfs,
                true,
                crate::domain::PermissionInheritanceMode::InheritParent,
            )
            .await
            {
                let _ = dst_fs.set_permissions(&dst_vfs, &perms).await;
            }

            // 2. Create all subdirectories first
            for item in items.iter().filter(|i| i.is_dir) {
                if cancel_token.is_cancelled() {
                    return Err(anyhow::anyhow!("Transfer cancelled by user"));
                }
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
                        return Err(anyhow::anyhow!(
                            "Failed creating subdirectory '{}': {}",
                            dst_dir_vfs.path,
                            e
                        ));
                    }
                }
                if let Some(perms) = crate::domain::resolve_destination_permissions(
                    &dst_fs,
                    &dst_dir_vfs,
                    true,
                    crate::domain::PermissionInheritanceMode::InheritParent,
                )
                .await
                {
                    let _ = dst_fs.set_permissions(&dst_dir_vfs, &perms).await;
                }
            }

            // 3. Stream each file
            let mut transferred_so_far = 0u64;
            let start_time = Instant::now();

            job.phase = TransferPhase::Transferring;
            {
                let mut map = jobs_map.write().await;
                if let Some(j) = map.get_mut(&job.id) {
                    j.phase = TransferPhase::Transferring;
                }
            }

            for item in items.iter().filter(|i| !i.is_dir) {
                if cancel_token.is_cancelled() {
                    return Err(anyhow::anyhow!("Transfer cancelled by user"));
                }

                let src_file_vfs = VfsPath::new(
                    &job.source_connection_id,
                    format!("{}/{}", src_vfs.path.trim_end_matches('/'), item.rel_path),
                )?;
                let dst_file_vfs = VfsPath::new(
                    &job.destination_connection_id,
                    format!("{}/{}", dst_vfs.path.trim_end_matches('/'), item.rel_path),
                )?;

                let mut reader = src_fs
                    .read_stream(&src_file_vfs)
                    .await
                    .map_err(|e| anyhow::anyhow!("Read failed: {}", e))?;
                let (mut pipe_writer, pipe_reader) = tokio::io::duplex(64 * 1024);

                let job_id = job.id.clone();
                let jobs_map_clone = Arc::clone(jobs_map);
                let event_tx_clone = event_tx.clone();
                let seq_counter_clone = Arc::clone(seq_counter);
                let event_hist_clone = Arc::clone(event_history);
                let file_size = item.size;
                let current_base_transferred = transferred_so_far;
                let cancel_token_pump = cancel_token.clone();

                let pump_handle = tokio::spawn(async move {
                    let mut buffer = vec![0u8; 64 * 1024];
                    let mut file_transferred = 0u64;
                    let mut last_emit = Instant::now();

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

                        tokio::select! {
                            _ = cancel_token_pump.cancelled() => {
                                return Err(anyhow::anyhow!("Transfer cancelled by user"));
                            }
                            res = pipe_writer.write_all(&buffer[..n]) => res?,
                        };

                        file_transferred += n as u64;

                        let total_transferred = current_base_transferred + file_transferred;
                        if last_emit.elapsed().as_millis() >= 100 || file_transferred == file_size {
                            let elapsed_secs = start_time.elapsed().as_secs_f64().max(0.001);
                            let speed = (total_transferred as f64 / elapsed_secs) as u64;
                            let eta = if speed > 0 && total_bytes > total_transferred {
                                Some((total_bytes - total_transferred) / speed)
                            } else {
                                Some(0)
                            };

                            let updated = {
                                let mut map = jobs_map_clone.write().await;
                                if let Some(j) = map.get_mut(&job_id) {
                                    j.transferred_bytes = total_transferred;
                                    j.total_bytes = total_bytes;
                                    j.speed_bytes_per_sec = speed;
                                    j.eta_seconds = eta;
                                    j.phase = TransferPhase::Transferring;
                                    j.updated_at = Utc::now();
                                    Some(j.clone())
                                } else {
                                    None
                                }
                            };

                            if let Some(j) = updated {
                                Self::send_enveloped_event(
                                    &event_tx_clone,
                                    &seq_counter_clone,
                                    &event_hist_clone,
                                    WsEvent::TransferProgress(j.clone()),
                                );
                            }
                            last_emit = Instant::now();
                        }
                    }

                    pipe_writer.flush().await?;
                    drop(pipe_writer);
                    Ok::<u64, anyhow::Error>(file_transferred)
                });

                let write_fut = dst_fs.write_stream(&dst_file_vfs, Box::new(pipe_reader));
                let write_res = tokio::select! {
                    _ = cancel_token.cancelled() => {
                        let _ = dst_fs.delete(&dst_file_vfs).await;
                        return Err(anyhow::anyhow!("Transfer cancelled by user"));
                    }
                    res = tokio::time::timeout(Duration::from_secs(120), write_fut) => {
                        res.map_err(|_| {
                            anyhow::anyhow!(
                                "Destination write stream timed out for file '{}'",
                                item.rel_path
                            )
                        })?
                    }
                };

                let pump_res = tokio::select! {
                    _ = cancel_token.cancelled() => {
                        let _ = dst_fs.delete(&dst_file_vfs).await;
                        return Err(anyhow::anyhow!("Transfer cancelled by user"));
                    }
                    res = pump_handle => {
                        res.map_err(|e| anyhow::anyhow!("Pump panic: {}", e))?
                    }
                };
                let file_bytes = pump_res?;
                write_res.map_err(|e| anyhow::anyhow!("Write failed: {}", e))?;

                if let Some(perms) = crate::domain::resolve_destination_permissions(
                    &dst_fs,
                    &dst_file_vfs,
                    false,
                    crate::domain::PermissionInheritanceMode::InheritExistingOrParent,
                )
                .await
                {
                    let _ = dst_fs.set_permissions(&dst_file_vfs, &perms).await;
                }

                transferred_so_far += file_bytes;

                // Move transfer: delete source file
                if job.transfer_type == TransferType::Move {
                    src_fs.delete(&src_file_vfs).await.map_err(|e| {
                        anyhow::anyhow!("Failed to delete source file {}: {}", src_file_vfs.path, e)
                    })?;
                }
            }

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
                Self::send_enveloped_event(
                    event_tx,
                    seq_counter,
                    event_history,
                    WsEvent::TransferProgress(job.clone()),
                );

                src_fs.delete(&src_vfs).await.map_err(|e| {
                    anyhow::anyhow!("Failed to delete source directory {}: {}", src_vfs.path, e)
                })?;
            }

            job.transferred_bytes = total_bytes;
            job.total_bytes = total_bytes;
            job.phase = TransferPhase::Completed;
            job.speed_bytes_per_sec = 0;
            job.eta_seconds = Some(0);
            job.updated_at = Utc::now();
            return Ok(());
        }

        // Single File Transfer with In-Flight Checksum Calculation
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

        // 2. Open source async read stream
        let mut reader = src_fs
            .read_stream(&src_vfs)
            .await
            .map_err(|e| anyhow::anyhow!("Read stream failed: {}", e))?;

        // 3. Create a 64 KB bounded duplex async pipe (Zero huge RAM allocations)
        let (mut pipe_writer, pipe_reader) = tokio::io::duplex(64 * 1024);

        let job_id = job.id.clone();
        let total_bytes = job.total_bytes;
        let jobs_map_clone = Arc::clone(jobs_map);
        let event_tx_clone = event_tx.clone();
        let seq_counter_clone = Arc::clone(seq_counter);
        let event_hist_clone = Arc::clone(event_history);
        let db_clone = db.clone();
        let cancel_token_pump = cancel_token.clone();

        job.phase = TransferPhase::Transferring;
        {
            let mut map = jobs_map.write().await;
            if let Some(j) = map.get_mut(&job.id) {
                j.phase = TransferPhase::Transferring;
            }
        }

        // 4. Spawn writer task to pump data, calculate SHA-256 checksum on-the-fly, and write to pipe
        let pump_handle = tokio::spawn(async move {
            let mut buffer = vec![0u8; 64 * 1024];
            let mut transferred = 0u64;
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

                // Streaming SHA-256 hash update
                hasher.update(&buffer[..n]);

                // Write chunk into bounded pipe (awaits if destination consumer is slower)
                tokio::select! {
                    _ = cancel_token_pump.cancelled() => {
                        return Err(anyhow::anyhow!("Transfer cancelled by user"));
                    }
                    res = pipe_writer.write_all(&buffer[..n]) => res?,
                };
                transferred += n as u64;

                // Update metrics & emit progress events
                if last_emit.elapsed().as_millis() >= 100 || transferred == total_bytes {
                    let elapsed_secs = start_time.elapsed().as_secs_f64().max(0.001);
                    let speed = (transferred as f64 / elapsed_secs) as u64;
                    let eta = if speed > 0 && total_bytes > transferred {
                        Some((total_bytes - transferred) / speed)
                    } else {
                        Some(0)
                    };

                    let updated_job = {
                        let mut map = jobs_map_clone.write().await;
                        if let Some(j) = map.get_mut(&job_id) {
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
                        } else {
                            None
                        }
                    };

                    if let Some(j) = updated_job {
                        Self::send_enveloped_event(
                            &event_tx_clone,
                            &seq_counter_clone,
                            &event_hist_clone,
                            WsEvent::TransferProgress(j.clone()),
                        );

                        // Persist to DB every 2 seconds or on finish
                        if last_db_save.elapsed().as_secs() >= 2 || transferred == total_bytes {
                            let _ = Self::save_job_to_db(&db_clone, &j).await;
                            last_db_save = Instant::now();
                        }
                    }

                    last_emit = Instant::now();
                }
            }

            pipe_writer.flush().await?;
            drop(pipe_writer); // Signal EOF to destination consumer

            // Emit Finalizing phase explicitly to UI
            {
                let mut map = jobs_map_clone.write().await;
                if let Some(j) = map.get_mut(&job_id) {
                    j.transferred_bytes = transferred;
                    j.phase = TransferPhase::Finalizing;
                    j.speed_bytes_per_sec = 0;
                    j.eta_seconds = Some(0);
                    j.updated_at = Utc::now();
                }
            }
            Self::send_enveloped_event(
                &event_tx_clone,
                &seq_counter_clone,
                &event_hist_clone,
                WsEvent::TransferProgress({
                    let map = jobs_map_clone.read().await;
                    map.get(&job_id).cloned().unwrap()
                }),
            );

            let checksum_hex = hex::encode(hasher.finalize());
            Ok::<(u64, String), anyhow::Error>((transferred, checksum_hex))
        });

        // 5. Destination writes into an isolated hidden .part staging file
        let parent = std::path::Path::new(&dst_vfs.path)
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("");
        let filename = std::path::Path::new(&dst_vfs.path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file");
        let staging_path = if parent.is_empty() || parent == "/" {
            format!("/.{}.aerofs-part-{}", filename, job.id)
        } else {
            format!(
                "{}/.{}.aerofs-part-{}",
                parent.trim_end_matches('/'),
                filename,
                job.id
            )
        };
        let part_vfs = VfsPath::new(&job.destination_connection_id, staging_path)?;

        // Best effort cleanup of any stale part file from previous attempt
        let _ = dst_fs.delete(&part_vfs).await;

        let write_fut = dst_fs.write_stream(&part_vfs, Box::new(pipe_reader));
        let write_res = tokio::select! {
            _ = cancel_token.cancelled() => {
                let _ = dst_fs.delete(&part_vfs).await;
                return Err(anyhow::anyhow!("Transfer cancelled by user"));
            }
            res = tokio::time::timeout(Duration::from_secs(120), write_fut) => {
                res.map_err(|_| anyhow::anyhow!("Destination write stream timed out after 120s"))?
            }
        };

        let pump_res = tokio::select! {
            _ = cancel_token.cancelled() => {
                let _ = dst_fs.delete(&part_vfs).await;
                return Err(anyhow::anyhow!("Transfer cancelled by user"));
            }
            res = pump_handle => {
                res.map_err(|e| anyhow::anyhow!("Stream pump task panicked: {}", e))?
            }
        };

        let (transferred_bytes, checksum) = match pump_res {
            Ok(val) => val,
            Err(e) => {
                let _ = dst_fs.delete(&part_vfs).await;
                return Err(e);
            }
        };

        if let Err(e) = write_res {
            let _ = dst_fs.delete(&part_vfs).await;
            return Err(anyhow::anyhow!("Destination write failed: {}", e));
        }

        // Atomically promote part file to destination path (strict error handling)
        if let Err(e) = dst_fs.rename(&part_vfs, &dst_vfs).await {
            let _ = dst_fs.delete(&part_vfs).await;
            return Err(anyhow::anyhow!(
                "Failed to promote staging file to final destination '{}': {}",
                dst_vfs.path,
                e
            ));
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
        job.checksum = Some(checksum.clone());

        // 6. Verification Phase
        job.phase = TransferPhase::Verifying;
        {
            let mut map = jobs_map.write().await;
            if let Some(j) = map.get_mut(&job.id) {
                j.phase = TransferPhase::Verifying;
                j.transferred_bytes = transferred_bytes;
                j.checksum = Some(checksum);
                j.updated_at = Utc::now();
            }
        }
        Self::send_enveloped_event(
            event_tx,
            seq_counter,
            event_history,
            WsEvent::TransferProgress(job.clone()),
        );

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
            Self::send_enveloped_event(
                event_tx,
                seq_counter,
                event_history,
                WsEvent::TransferProgress(job.clone()),
            );

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

    /// Save or update a transfer job in SQLite
    async fn save_job_to_db(db: &DbPool, job: &TransferJob) -> anyhow::Result<()> {
        let created_at = job.created_at.to_rfc3339();
        let updated_at = job.updated_at.to_rfc3339();
        let dismissed_at = job.dismissed_at.map(|d| d.to_rfc3339());

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

        Ok(())
    }
}
