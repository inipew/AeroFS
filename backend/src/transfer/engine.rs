use crate::db::DbPool;
use crate::domain::VfsPath;
use crate::vfs::FileSystem;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};
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

    pub fn from_str(s: &str) -> Self {
        match s {
            "move" => TransferType::Move,
            "upload" => TransferType::Upload,
            "sync" => TransferType::Sync,
            _ => TransferType::Copy,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TransferStatus {
    Queued,
    Running,
    CancellationRequested,
    Cancelled,
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
            TransferStatus::Completed => "completed",
            TransferStatus::Failed => "failed",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "running" => TransferStatus::Running,
            "cancellation_requested" => TransferStatus::CancellationRequested,
            "cancelled" => TransferStatus::Cancelled,
            "completed" => TransferStatus::Completed,
            "failed" => TransferStatus::Failed,
            _ => TransferStatus::Queued,
        }
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
    queue_tx: mpsc::Sender<String>,
    event_tx: broadcast::Sender<EventEnvelope>,
    sequence_counter: Arc<AtomicU64>,
    db: DbPool,
}

impl TransferManager {
    pub fn new(
        providers: Arc<RwLock<HashMap<String, Arc<dyn FileSystem>>>>,
        db: DbPool,
    ) -> Self {
        let (queue_tx, queue_rx) = mpsc::channel::<String>(200);
        let (event_tx, _) = broadcast::channel::<EventEnvelope>(400);

        let jobs: Arc<RwLock<HashMap<String, TransferJob>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let sequence_counter = Arc::new(AtomicU64::new(1));

        let jobs_clone = Arc::clone(&jobs);
        let event_tx_clone = event_tx.clone();
        let db_clone = db.clone();
        let queue_tx_clone = queue_tx.clone();
        let sequence_counter_clone = Arc::clone(&sequence_counter);

        // 1. Initial startup recovery: Load jobs from SQLite into memory
        let db_init = db.clone();
        let jobs_init = Arc::clone(&jobs);
        tokio::spawn(async move {
            if let Ok(saved_jobs) = Self::load_jobs_from_db(&db_init).await {
                let mut map = jobs_init.write().await;
                for mut job in saved_jobs {
                    // Mark interrupted 'running' jobs as failed on restart
                    if job.status == TransferStatus::Running {
                        job.status = TransferStatus::Failed;
                        job.error_message = Some("Transfer interrupted by server restart".into());
                        let _ = Self::save_job_to_db(&db_init, &job).await;
                    } else if job.status == TransferStatus::Queued {
                        let _ = queue_tx_clone.send(job.id.clone()).await;
                    }
                    map.insert(job.id.clone(), job);
                }
            }
        });

        // 2. Multi-Worker Concurrent Transfer Scheduler (Pool of 3 workers)
        let queue_rx_shared = Arc::new(Mutex::new(queue_rx));
        let max_concurrent_workers = 3;

        for _ in 0..max_concurrent_workers {
            let rx_worker = Arc::clone(&queue_rx_shared);
            let jobs_worker = Arc::clone(&jobs_clone);
            let providers_worker = Arc::clone(&providers);
            let event_tx_worker = event_tx_clone.clone();
            let seq_worker = Arc::clone(&sequence_counter_clone);
            let db_worker = db_clone.clone();

            tokio::spawn(async move {
                loop {
                    let job_id = {
                        let mut rx = rx_worker.lock().await;
                        rx.recv().await
                    };

                    let Some(job_id) = job_id else {
                        break;
                    };

                    let job_opt = {
                        let map = jobs_worker.read().await;
                        map.get(&job_id).cloned()
                    };

                    if let Some(mut job) = job_opt {
                        // Skip cancelled or already finished jobs
                        if job.status == TransferStatus::Cancelled
                            || job.status == TransferStatus::CancellationRequested
                        {
                            continue;
                        }

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
                            WsEvent::TransferProgress(job.clone()),
                        );

                        // Execute robust bounded-stream transfer with retry
                        let result = Self::execute_job_with_retry(
                            &mut job,
                            &providers_worker,
                            &jobs_worker,
                            &event_tx_worker,
                            &seq_worker,
                            &db_worker,
                        )
                        .await;

                        // Re-read fresh status in case user requested cancellation during transfer
                        let current_status = {
                            let map = jobs_worker.read().await;
                            map.get(&job.id).map(|j| j.status).unwrap_or(job.status)
                        };

                        if current_status == TransferStatus::Cancelled
                            || current_status == TransferStatus::CancellationRequested
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
                                WsEvent::TransferFailed(job),
                            );
                            continue;
                        }

                        match result {
                            Ok(()) => {
                                job.status = TransferStatus::Completed;
                                job.speed_bytes_per_sec = 0;
                                job.eta_seconds = Some(0);
                                job.updated_at = Utc::now();
                                {
                                    let mut map = jobs_worker.write().await;
                                    map.insert(job.id.clone(), job.clone());
                                }
                                let _ = Self::save_job_to_db(&db_worker, &job).await;
                                Self::send_enveloped_event(
                                    &event_tx_worker,
                                    &seq_worker,
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
                                    WsEvent::TransferFailed(job),
                                );
                            }
                        }
                    }
                }
            });
        }

        Self {
            jobs,
            queue_tx,
            event_tx,
            sequence_counter,
            db,
        }
    }

    fn send_enveloped_event(
        event_tx: &broadcast::Sender<EventEnvelope>,
        seq_counter: &Arc<AtomicU64>,
        event: WsEvent,
    ) {
        let envelope = EventEnvelope {
            id: Uuid::new_v4().to_string(),
            sequence: seq_counter.fetch_add(1, Ordering::SeqCst),
            timestamp: Utc::now(),
            event,
        };
        let _ = event_tx.send(envelope);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EventEnvelope> {
        self.event_tx.subscribe()
    }

    pub fn broadcast_event(&self, event: WsEvent) {
        Self::send_enveloped_event(&self.event_tx, &self.sequence_counter, event);
    }

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

        // 2. Insert into in-memory state
        {
            let mut map = self.jobs.write().await;
            map.insert(id.clone(), job.clone());
        }

        Self::send_enveloped_event(
            &self.event_tx,
            &self.sequence_counter,
            WsEvent::TransferProgress(job),
        );
        self.queue_tx
            .send(id.clone())
            .await
            .map_err(|e| format!("Failed to queue transfer job: {}", e))?;

        Ok(id)
    }

    pub async fn list_jobs(&self, user_id: Option<&str>, is_admin: bool, include_dismissed: bool) -> Vec<TransferJob> {
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
                    (None, _) => true,
                    _ => false,
                }
            })
            .cloned()
            .collect();
        list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        list
    }

    pub async fn cancel_job(&self, id: &str, user_id: Option<&str>, is_admin: bool) -> Result<bool, String> {
        let updated_job = {
            let mut map = self.jobs.write().await;
            if let Some(job) = map.get_mut(id) {
                if !is_admin {
                    if let (Some(owner), Some(uid)) = (&job.user_id, user_id) {
                        if owner != uid {
                            return Err("Permission denied: cannot cancel another user's transfer".into());
                        }
                    }
                }
                if job.status == TransferStatus::Queued
                    || job.status == TransferStatus::Running
                    || job.status == TransferStatus::CancellationRequested
                {
                    job.status = TransferStatus::Cancelled;
                    job.updated_at = Utc::now();
                    Some(job.clone())
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some(job) = updated_job {
            let _ = Self::save_job_to_db(&self.db, &job).await;
            Self::send_enveloped_event(
                &self.event_tx,
                &self.sequence_counter,
                WsEvent::TransferFailed(job),
            );
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub async fn dismiss_job(&self, id: &str, user_id: Option<&str>, is_admin: bool) -> Result<bool, String> {
        let updated_job = {
            let mut map = self.jobs.write().await;
            if let Some(job) = map.get_mut(id) {
                if !is_admin {
                    if let (Some(owner), Some(uid)) = (&job.user_id, user_id) {
                        if owner != uid {
                            return Err("Permission denied: cannot dismiss another user's transfer".into());
                        }
                    }
                }
                job.dismissed_at = Some(Utc::now());
                job.updated_at = Utc::now();
                Some(job.clone())
            } else {
                None
            }
        };

        if let Some(job) = updated_job {
            let _ = Self::save_job_to_db(&self.db, &job).await;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub async fn clear_finished_jobs(&self, user_id: Option<&str>, is_admin: bool) -> Result<usize, String> {
        let now = Utc::now();
        let mut count = 0;
        let mut jobs_to_persist = Vec::new();

        {
            let mut map = self.jobs.write().await;
            for job in map.values_mut() {
                if job.dismissed_at.is_none()
                    && (job.status == TransferStatus::Completed
                        || job.status == TransferStatus::Failed
                        || job.status == TransferStatus::Cancelled)
                {
                    let is_owner = if is_admin {
                        true
                    } else {
                        match (&job.user_id, user_id) {
                            (Some(owner), Some(uid)) => owner == uid,
                            (None, _) => true,
                            _ => false,
                        }
                    };

                    if is_owner {
                        job.dismissed_at = Some(now);
                        job.updated_at = now;
                        jobs_to_persist.push(job.clone());
                        count += 1;
                    }
                }
            }
        }

        for j in &jobs_to_persist {
            let _ = Self::save_job_to_db(&self.db, j).await;
        }

        Ok(count)
    }

    /// Execute transfer with exponential backoff retry for transient network hiccups
    async fn execute_job_with_retry(
        job: &mut TransferJob,
        providers: &Arc<RwLock<HashMap<String, Arc<dyn FileSystem>>>>,
        jobs_map: &Arc<RwLock<HashMap<String, TransferJob>>>,
        event_tx: &broadcast::Sender<EventEnvelope>,
        seq_counter: &Arc<AtomicU64>,
        db: &DbPool,
    ) -> anyhow::Result<()> {
        let max_attempts = 3;
        let mut attempt = 0;

        loop {
            attempt += 1;
            match Self::execute_job(job, providers, jobs_map, event_tx, seq_counter, db).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    // Check if job was cancelled
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

                    // Classify permanent errors (404, 403, 401, Invalid Path, Unsupported)
                    let err_msg = e.to_string().to_lowercase();
                    let is_permanent = err_msg.contains("not found")
                        || err_msg.contains("permission denied")
                        || err_msg.contains("forbidden")
                        || err_msg.contains("unauthorized")
                        || err_msg.contains("invalid path")
                        || err_msg.contains("unsupported");

                    if is_permanent || attempt >= max_attempts {
                        return Err(e);
                    }

                    let backoff = Duration::from_secs(1 << (attempt - 1)); // 1s, 2s, 4s
                    tracing::warn!(
                        "Transfer {} attempt {}/{} failed ({}), retrying in {:?}",
                        job.id,
                        attempt,
                        max_attempts,
                        e,
                        backoff
                    );
                    tokio::time::sleep(backoff).await;
                }
            }
        }
    }

    /// True Bounded-Buffer Asynchronous Streaming Transfer with SHA-256 Checksum Calculation
    async fn execute_job(
        job: &mut TransferJob,
        providers: &Arc<RwLock<HashMap<String, Arc<dyn FileSystem>>>>,
        jobs_map: &Arc<RwLock<HashMap<String, TransferJob>>>,
        event_tx: &broadcast::Sender<EventEnvelope>,
        seq_counter: &Arc<AtomicU64>,
        db: &DbPool,
    ) -> anyhow::Result<()> {
        let src_fs = {
            let p = providers.read().await;
            p.get(&job.source_connection_id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Source connection '{}' not found", job.source_connection_id))?
        };

        let dst_fs = {
            let p = providers.read().await;
            p.get(&job.destination_connection_id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Destination connection '{}' not found", job.destination_connection_id))?
        };

        let src_vfs = VfsPath::new(&job.source_connection_id, &job.source_path);
        let dst_vfs = VfsPath::new(&job.destination_connection_id, &job.destination_path);

        // 1. Get source metadata
        let meta = src_fs
            .stat(&src_vfs)
            .await
            .map_err(|e| anyhow::anyhow!("Stat source failed: {}", e))?;

        // FAST-PATH: Native atomic rename for same-connection Move (preserves inode, permissions, timestamps, instant)
        if job.transfer_type == TransferType::Move && job.source_connection_id == job.destination_connection_id {
            match src_fs.rename(&src_vfs, &dst_vfs).await {
                Ok(_) => {
                    job.transferred_bytes = meta.size;
                    job.total_bytes = meta.size;
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
            // Recursive directory transfer!
            #[derive(Debug)]
            struct ItemToTransfer {
                rel_path: String,
                is_dir: bool,
                size: u64,
            }

            async fn scan_dir_recursive(
                fs: &Arc<dyn FileSystem>,
                conn_id: &str,
                base_vfs: &VfsPath,
                current_rel: &str,
                items: &mut Vec<ItemToTransfer>,
            ) -> anyhow::Result<()> {
                let current_vfs = if current_rel.is_empty() {
                    base_vfs.clone()
                } else {
                    VfsPath::new(conn_id, format!("{}/{}", base_vfs.path.trim_end_matches('/'), current_rel))
                };

                let entries = fs.list(&current_vfs).await.map_err(|e| anyhow::anyhow!("List failed: {}", e))?;
                for entry in entries {
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
                        Box::pin(scan_dir_recursive(fs, conn_id, base_vfs, &child_rel, items)).await?;
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
            scan_dir_recursive(&src_fs, &job.source_connection_id, &src_vfs, "", &mut items).await?;

            let total_bytes: u64 = items.iter().map(|i| i.size).sum();
            job.total_bytes = total_bytes;
            job.transferred_bytes = 0;

            // 1. Create root destination directory
            dst_fs.create_dir(&dst_vfs).await.map_err(|e| anyhow::anyhow!("Failed creating destination dir: {}", e))?;

            // 2. Create all subdirectories first
            for item in items.iter().filter(|i| i.is_dir) {
                let dst_dir_vfs = VfsPath::new(&job.destination_connection_id, format!("{}/{}", dst_vfs.path.trim_end_matches('/'), item.rel_path));
                let _ = dst_fs.create_dir(&dst_dir_vfs).await;
            }

            // 3. Stream each file
            let mut transferred_so_far = 0u64;
            let start_time = Instant::now();

            for item in items.iter().filter(|i| !i.is_dir) {
                // Check cancellation
                {
                    let map = jobs_map.read().await;
                    if let Some(j) = map.get(&job.id) {
                        if j.status == TransferStatus::Cancelled
                            || j.status == TransferStatus::CancellationRequested
                        {
                            return Err(anyhow::anyhow!("Transfer cancelled"));
                        }
                    }
                }

                let src_file_vfs = VfsPath::new(&job.source_connection_id, format!("{}/{}", src_vfs.path.trim_end_matches('/'), item.rel_path));
                let dst_file_vfs = VfsPath::new(&job.destination_connection_id, format!("{}/{}", dst_vfs.path.trim_end_matches('/'), item.rel_path));

                let mut reader = src_fs.read_stream(&src_file_vfs).await.map_err(|e| anyhow::anyhow!("Read failed: {}", e))?;
                let (mut pipe_writer, pipe_reader) = tokio::io::duplex(64 * 1024);

                let job_id = job.id.clone();
                let jobs_map_clone = Arc::clone(jobs_map);
                let event_tx_clone = event_tx.clone();
                let seq_counter_clone = Arc::clone(seq_counter);
                let file_size = item.size;
                let current_base_transferred = transferred_so_far;

                let pump_handle = tokio::spawn(async move {
                    let mut buffer = vec![0u8; 64 * 1024];
                    let mut file_transferred = 0u64;
                    let mut last_emit = Instant::now();

                    loop {
                        let n = reader.read(&mut buffer).await?;
                        if n == 0 {
                            break;
                        }
                        pipe_writer.write_all(&buffer[..n]).await?;
                        file_transferred += n as u64;

                        let total_transferred = current_base_transferred + file_transferred;
                        if last_emit.elapsed().as_millis() >= 200 || file_transferred == file_size {
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

                let existing_dst_perms = dst_fs.stat(&dst_file_vfs).await.ok().and_then(|m| m.permissions);
                let write_res = dst_fs.write_stream(&dst_file_vfs, Box::new(pipe_reader)).await;
                let pump_res = pump_handle.await.map_err(|e| anyhow::anyhow!("Pump panic: {}", e))?;
                let file_bytes = pump_res?;
                write_res.map_err(|e| anyhow::anyhow!("Write failed: {}", e))?;

                if let Some(ref perms) = existing_dst_perms {
                    let _ = dst_fs.set_permissions(&dst_file_vfs, perms).await;
                }

                transferred_so_far += file_bytes;
            }

            job.transferred_bytes = transferred_so_far;

            if job.transfer_type == TransferType::Move {
                src_fs.delete(&src_vfs).await.map_err(|e| anyhow::anyhow!("Move cleanup failed: {}", e))?;
            }

            return Ok(());
        }

        job.total_bytes = meta.size;
        job.transferred_bytes = 0;

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
        let db_clone = db.clone();

        // 4. Spawn writer task to pump data, calculate SHA-256 checksum on-the-fly, and write to pipe
        let pump_handle = tokio::spawn(async move {
            let mut buffer = vec![0u8; 64 * 1024];
            let mut transferred = 0u64;
            let mut hasher = Sha256::new();
            let start_time = Instant::now();
            let mut last_emit = Instant::now();
            let mut last_db_save = Instant::now();

            loop {
                // Check cancellation state machine
                {
                    let map = jobs_map_clone.read().await;
                    if let Some(j) = map.get(&job_id) {
                        if j.status == TransferStatus::Cancelled
                            || j.status == TransferStatus::CancellationRequested
                        {
                            return Err(anyhow::anyhow!("Transfer cancelled"));
                        }
                    }
                }

                let n = reader.read(&mut buffer).await?;
                if n == 0 {
                    break;
                }

                // Streaming SHA-256 hash update
                hasher.update(&buffer[..n]);

                // Write chunk into bounded pipe (awaits if destination consumer is slower)
                pipe_writer.write_all(&buffer[..n]).await?;
                transferred += n as u64;

                // Update metrics & emit progress events
                if last_emit.elapsed().as_millis() >= 200 || transferred == total_bytes {
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

            let checksum_hex = hex::encode(hasher.finalize());
            Ok::<(u64, String), anyhow::Error>((transferred, checksum_hex))
        });

        // 5. Destination writes directly from the streaming pipe
        let existing_dst_perms = dst_fs.stat(&dst_vfs).await.ok().and_then(|m| m.permissions);
        let write_res = dst_fs
            .write_stream(&dst_vfs, Box::new(pipe_reader))
            .await;

        let pump_res = pump_handle
            .await
            .map_err(|e| anyhow::anyhow!("Stream pump task panicked: {}", e))?;

        let (transferred_bytes, checksum) = pump_res?;
        write_res.map_err(|e| anyhow::anyhow!("Destination write failed: {}", e))?;

        if let Some(ref perms) = existing_dst_perms {
            let _ = dst_fs.set_permissions(&dst_vfs, perms).await;
        }

        job.transferred_bytes = transferred_bytes;
        job.checksum = Some(checksum);

        // 6. Transactional Move: verify integrity before deleting source
        if job.transfer_type == TransferType::Move {
            if transferred_bytes < job.total_bytes {
                return Err(anyhow::anyhow!(
                    "Move aborted: transferred bytes ({}) does not match source size ({})",
                    transferred_bytes,
                    job.total_bytes
                ));
            }

            // Verify destination file exists
            if dst_fs.stat(&dst_vfs).await.is_err() {
                return Err(anyhow::anyhow!(
                    "Move aborted: failed to verify destination file before deleting source"
                ));
            }

            // Safely delete source file
            src_fs
                .delete(&src_vfs)
                .await
                .map_err(|e| anyhow::anyhow!("Move completed with cleanup failure: {}", e))?;
        }

        Ok(())
    }

    /// Load transfer jobs from SQLite
    async fn load_jobs_from_db(db: &DbPool) -> anyhow::Result<Vec<TransferJob>> {
        let rows = sqlx::query(
            "SELECT id, user_id, name, transfer_type, source_connection_id, source_path,
                    destination_connection_id, destination_path, status,
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
            let transferred_bytes: i64 = r.get("transferred_bytes");
            let total_bytes: i64 = r.get("total_bytes");
            let speed_bytes_per_sec: i64 = r.get("speed_bytes_per_sec");
            let eta_seconds: Option<i64> = r.get("eta_seconds");
            let checksum: Option<String> = r.get("checksum");
            let error_message: Option<String> = r.get("error_message");
            let dismissed_at_str: Option<String> = r.get("dismissed_at");
            let created_at_str: String = r.get("created_at");
            let updated_at_str: String = r.get("updated_at");

            let dismissed_at = dismissed_at_str.and_then(|d| DateTime::parse_from_rfc3339(&d).ok().map(|dt| dt.with_timezone(&Utc)));
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

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
                destination_connection_id, destination_path, status,
                transferred_bytes, total_bytes, speed_bytes_per_sec,
                eta_seconds, checksum, error_message, dismissed_at, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                status = excluded.status,
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
