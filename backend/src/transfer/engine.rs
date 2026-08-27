use crate::domain::VfsPath;
use crate::vfs::FileSystem;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::AsyncReadExt;
use tokio::sync::{broadcast, mpsc, RwLock};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TransferStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TransferJob {
    pub id: String,
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
    pub error_message: Option<String>,
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
    FileChange { connection_id: String, path: String, action: String },
}

#[derive(Clone)]
pub struct TransferManager {
    jobs: Arc<RwLock<HashMap<String, TransferJob>>>,
    queue_tx: mpsc::Sender<String>,
    event_tx: broadcast::Sender<WsEvent>,
}

impl TransferManager {
    pub fn new(providers: Arc<RwLock<HashMap<String, Arc<dyn FileSystem>>>>) -> Self {
        let (queue_tx, mut queue_rx) = mpsc::channel::<String>(100);
        let (event_tx, _) = broadcast::channel::<WsEvent>(200);

        let jobs: Arc<RwLock<HashMap<String, TransferJob>>> = Arc::new(RwLock::new(HashMap::new()));
        let jobs_clone = Arc::clone(&jobs);
        let event_tx_clone = event_tx.clone();

        // Background worker loop
        tokio::spawn(async move {
            while let Some(job_id) = queue_rx.recv().await {
                let job_opt = {
                    let map = jobs_clone.read().await;
                    map.get(&job_id).cloned()
                };

                if let Some(mut job) = job_opt {
                    if job.status == TransferStatus::Cancelled {
                        continue;
                    }

                    job.status = TransferStatus::Running;
                    job.updated_at = Utc::now();
                    {
                        let mut map = jobs_clone.write().await;
                        map.insert(job.id.clone(), job.clone());
                    }
                    let _ = event_tx_clone.send(WsEvent::TransferProgress(job.clone()));

                    // Execute transfer logic
                    let result = Self::execute_job(&mut job, &providers, &jobs_clone, &event_tx_clone).await;

                    match result {
                        Ok(()) => {
                            job.status = TransferStatus::Completed;
                            job.speed_bytes_per_sec = 0;
                            job.eta_seconds = Some(0);
                            job.updated_at = Utc::now();
                            let mut map = jobs_clone.write().await;
                            map.insert(job.id.clone(), job.clone());
                            let _ = event_tx_clone.send(WsEvent::TransferCompleted(job));
                        }
                        Err(e) => {
                            if job.status != TransferStatus::Cancelled {
                                job.status = TransferStatus::Failed;
                                job.error_message = Some(e.to_string());
                                job.updated_at = Utc::now();
                                let mut map = jobs_clone.write().await;
                                map.insert(job.id.clone(), job.clone());
                                let _ = event_tx_clone.send(WsEvent::TransferFailed(job));
                            }
                        }
                    }
                }
            }
        });

        Self {
            jobs,
            queue_tx,
            event_tx,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WsEvent> {
        self.event_tx.subscribe()
    }

    pub fn broadcast_event(&self, event: WsEvent) {
        let _ = self.event_tx.send(event);
    }

    pub async fn submit_job(
        &self,
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
            error_message: None,
            created_at: now,
            updated_at: now,
        };

        {
            let mut map = self.jobs.write().await;
            map.insert(id.clone(), job.clone());
        }

        let _ = self.event_tx.send(WsEvent::TransferProgress(job));
        self.queue_tx
            .send(id.clone())
            .await
            .map_err(|e| format!("Failed to queue transfer job: {}", e))?;

        Ok(id)
    }

    pub async fn list_jobs(&self) -> Vec<TransferJob> {
        let map = self.jobs.read().await;
        let mut list: Vec<TransferJob> = map.values().cloned().collect();
        list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        list
    }

    pub async fn cancel_job(&self, id: &str) -> bool {
        let mut map = self.jobs.write().await;
        if let Some(job) = map.get_mut(id) {
            if job.status == TransferStatus::Queued || job.status == TransferStatus::Running {
                job.status = TransferStatus::Cancelled;
                job.updated_at = Utc::now();
                let _ = self.event_tx.send(WsEvent::TransferFailed(job.clone()));
                return true;
            }
        }
        false
    }

    async fn execute_job(
        job: &mut TransferJob,
        providers: &Arc<RwLock<HashMap<String, Arc<dyn FileSystem>>>>,
        jobs_map: &Arc<RwLock<HashMap<String, TransferJob>>>,
        event_tx: &broadcast::Sender<WsEvent>,
    ) -> anyhow::Result<()> {
        let src_fs = {
            let p = providers.read().await;
            p.get(&job.source_connection_id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Source connection not found"))?
        };

        let dst_fs = {
            let p = providers.read().await;
            p.get(&job.destination_connection_id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Destination connection not found"))?
        };

        let src_vfs = VfsPath::new(&job.source_connection_id, &job.source_path);
        let dst_vfs = VfsPath::new(&job.destination_connection_id, &job.destination_path);

        // Get source metadata for total bytes
        let meta = src_fs.stat(&src_vfs).await.map_err(|e| anyhow::anyhow!("Stat source failed: {}", e))?;
        job.total_bytes = meta.size;

        // Open read stream
        let mut reader = src_fs
            .read_stream(&src_vfs)
            .await
            .map_err(|e| anyhow::anyhow!("Read stream failed: {}", e))?;

        // Buffer and progress tracking
        let mut buffer = vec![0u8; 64 * 1024]; // 64 KB chunk
        let mut transferred = 0u64;
        let start_time = Instant::now();
        let mut last_emit = Instant::now();
        let mut collected_bytes = Vec::new();

        loop {
            // Check for cancellation
            {
                let map = jobs_map.read().await;
                if let Some(j) = map.get(&job.id) {
                    if j.status == TransferStatus::Cancelled {
                        return Ok(());
                    }
                }
            }

            let n = reader.read(&mut buffer).await?;
            if n == 0 {
                break;
            }

            collected_bytes.extend_from_slice(&buffer[..n]);
            transferred += n as u64;
            job.transferred_bytes = transferred;

            // Calculate metrics every 250ms or at completion
            if last_emit.elapsed().as_millis() > 250 || transferred == job.total_bytes {
                let elapsed_secs = start_time.elapsed().as_secs_f64().max(0.001);
                let speed = (transferred as f64 / elapsed_secs) as u64;
                job.speed_bytes_per_sec = speed;

                if speed > 0 && job.total_bytes > transferred {
                    job.eta_seconds = Some((job.total_bytes - transferred) / speed);
                } else {
                    job.eta_seconds = Some(0);
                }

                job.updated_at = Utc::now();
                {
                    let mut map = jobs_map.write().await;
                    map.insert(job.id.clone(), job.clone());
                }
                let _ = event_tx.send(WsEvent::TransferProgress(job.clone()));
                last_emit = Instant::now();
            }
        }

        // Write to destination
        let cursor = std::io::Cursor::new(collected_bytes);
        dst_fs
            .write_stream(&dst_vfs, Box::new(cursor))
            .await
            .map_err(|e| anyhow::anyhow!("Write stream failed: {}", e))?;

        // If Move operation, delete source
        if job.transfer_type == TransferType::Move {
            let _ = src_fs.delete(&src_vfs).await;
        }

        Ok(())
    }
}
