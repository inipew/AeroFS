use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Row, Sqlite};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum DomainEvent {
    #[serde(rename = "transfer_progress")]
    TransferProgress(serde_json::Value),
    #[serde(rename = "transfer_completed")]
    TransferCompleted(serde_json::Value),
    #[serde(rename = "transfer_failed")]
    TransferFailed(serde_json::Value),
    #[serde(rename = "file_change")]
    FileChange {
        connection_id: String,
        path: String,
        action: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        old_path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        old_parent_path: Option<String>,
    },
    #[serde(rename = "permission_changed")]
    PermissionChanged {
        user_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        connection_id: Option<String>,
    },
    #[serde(rename = "resync_required")]
    ResyncRequired {
        reason: String,
        latest_sequence: u64,
    },
    #[serde(rename = "full_sync")]
    FullSync {
        reason: String,
        epoch: String,
    },
}

impl DomainEvent {
    pub fn file_change(
        connection_id: impl Into<String>,
        path: impl Into<String>,
        action: impl Into<String>,
    ) -> Self {
        let p: String = path.into();
        let parent = std::path::Path::new(&p).parent().map(|d| {
            let s = d.to_string_lossy().to_string();
            if s.is_empty() {
                "/".to_string()
            } else {
                s
            }
        });
        Self::FileChange {
            connection_id: connection_id.into(),
            path: p,
            action: action.into(),
            old_path: None,
            parent_path: parent,
            old_parent_path: None,
        }
    }

    pub fn file_rename(
        connection_id: impl Into<String>,
        from_path: impl Into<String>,
        to_path: impl Into<String>,
    ) -> Self {
        let from_str: String = from_path.into();
        let to_str: String = to_path.into();
        let old_parent = std::path::Path::new(&from_str).parent().map(|d| {
            let s = d.to_string_lossy().to_string();
            if s.is_empty() {
                "/".to_string()
            } else {
                s
            }
        });
        let parent = std::path::Path::new(&to_str).parent().map(|d| {
            let s = d.to_string_lossy().to_string();
            if s.is_empty() {
                "/".to_string()
            } else {
                s
            }
        });
        Self::FileChange {
            connection_id: connection_id.into(),
            path: to_str,
            action: "rename".into(),
            old_path: Some(from_str),
            parent_path: parent,
            old_parent_path: old_parent,
        }
    }

    pub fn event_type_name(&self) -> &'static str {
        match self {
            DomainEvent::TransferProgress(_) => "transfer_progress",
            DomainEvent::TransferCompleted(_) => "transfer_completed",
            DomainEvent::TransferFailed(_) => "transfer_failed",
            DomainEvent::FileChange { .. } => "file_change",
            DomainEvent::PermissionChanged { .. } => "permission_changed",
            DomainEvent::ResyncRequired { .. } => "resync_required",
            DomainEvent::FullSync { .. } => "full_sync",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub id: String,
    pub epoch: String,
    pub sequence: u64,
    pub timestamp: DateTime<Utc>,
    #[serde(flatten)]
    pub event: DomainEvent,
}

#[derive(Debug, Clone)]
pub enum ReplayOutcome {
    Events(Vec<EventEnvelope>),
    Expired { latest_sequence: u64 },
    EpochMismatch { current_epoch: String, latest_sequence: u64 },
}

/// Durable Event Journal backed by SQLite and real-time in-memory broadcast.
#[derive(Clone)]
pub struct EventJournal {
    db: Pool<Sqlite>,
    epoch: String,
    sequence_counter: Arc<AtomicU64>,
    event_tx: broadcast::Sender<EventEnvelope>,
}

impl EventJournal {
    /// Initialize EventJournal with a unique generation epoch and load initial sequence.
    pub async fn init(db: Pool<Sqlite>) -> anyhow::Result<Self> {
        let epoch = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        // Upsert server_epoch record
        let _ = sqlx::query(
            "INSERT INTO server_epoch (id, epoch, created_at) VALUES (1, ?, ?)
             ON CONFLICT(id) DO UPDATE SET epoch = excluded.epoch, created_at = excluded.created_at",
        )
        .bind(&epoch)
        .bind(&now)
        .execute(&db)
        .await;

        let (event_tx, _) = broadcast::channel::<EventEnvelope>(1000);

        Ok(Self {
            db,
            epoch,
            sequence_counter: Arc::new(AtomicU64::new(1)),
            event_tx,
        })
    }

    pub fn epoch(&self) -> &str {
        &self.epoch
    }

    pub fn latest_sequence(&self) -> u64 {
        self.sequence_counter.load(Ordering::SeqCst)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EventEnvelope> {
        self.event_tx.subscribe()
    }

    /// Append a domain event into the durable SQLite journal and broadcast it live to subscribers.
    pub async fn append(
        &self,
        event: DomainEvent,
        aggregate_id: Option<&str>,
    ) -> anyhow::Result<EventEnvelope> {
        let seq = self.sequence_counter.fetch_add(1, Ordering::SeqCst);
        let event_id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let now_str = now.to_rfc3339();

        let envelope = EventEnvelope {
            id: event_id.clone(),
            epoch: self.epoch.clone(),
            sequence: seq,
            timestamp: now,
            event: event.clone(),
        };

        // Broadcast to live WebSocket listeners
        let _ = self.event_tx.send(envelope.clone());

        // Don't persist transient high-frequency progress ticks to disk, only lifecycle and file change events
        let is_progress = matches!(event, DomainEvent::TransferProgress(_));
        if !is_progress {
            let payload = serde_json::to_string(&event)?;
            let event_type = event.event_type_name();
            let db = self.db.clone();
            let epoch_str = self.epoch.clone();
            let agg_id = aggregate_id.map(|s| s.to_string());
            
            // Asynchronously persist to SQLite event_journal
            tokio::spawn(async move {
                let _ = sqlx::query(
                    "INSERT INTO event_journal (epoch, sequence, event_type, aggregate_id, payload, created_at)
                     VALUES (?, ?, ?, ?, ?, ?)",
                )
                .bind(epoch_str)
                .bind(seq as i64)
                .bind(event_type)
                .bind(agg_id)
                .bind(payload)
                .bind(now_str)
                .execute(&db)
                .await;
            });
        }

        Ok(envelope)
    }

    /// Replay missed events starting from `last_sequence` within `client_epoch`.
    pub async fn get_since(
        &self,
        client_epoch: Option<&str>,
        last_sequence: u64,
        limit: usize,
    ) -> anyhow::Result<ReplayOutcome> {
        let current_latest = self.latest_sequence();

        // 1. Check if client has a different or missing epoch
        if let Some(ep) = client_epoch {
            if ep != self.epoch {
                return Ok(ReplayOutcome::EpochMismatch {
                    current_epoch: self.epoch.clone(),
                    latest_sequence: current_latest,
                });
            }
        } else {
            // Fresh connect without prior epoch
            return Ok(ReplayOutcome::EpochMismatch {
                current_epoch: self.epoch.clone(),
                latest_sequence: current_latest,
            });
        }

        // 2. Client is already up to date
        if last_sequence >= current_latest {
            return Ok(ReplayOutcome::Events(Vec::new()));
        }

        // 3. Query SQLite for persisted events in this epoch
        let rows = sqlx::query(
            "SELECT sequence, payload, created_at FROM event_journal
             WHERE epoch = ? AND sequence > ?
             ORDER BY sequence ASC LIMIT ?",
        )
        .bind(&self.epoch)
        .bind(last_sequence as i64)
        .bind(limit as i64)
        .fetch_all(&self.db)
        .await?;

        if rows.is_empty() {
            // If requested sequence is too old and purged
            if last_sequence + 1 < current_latest {
                return Ok(ReplayOutcome::Expired {
                    latest_sequence: current_latest,
                });
            }
            return Ok(ReplayOutcome::Events(Vec::new()));
        }

        // Verify sequence continuity: first row must be last_sequence + 1
        let first_seq: i64 = rows[0].get("sequence");
        if first_seq as u64 > last_sequence + 1 {
            return Ok(ReplayOutcome::Expired {
                latest_sequence: current_latest,
            });
        }

        let mut events = Vec::with_capacity(rows.len());
        for r in rows {
            let seq: i64 = r.get("sequence");
            let payload_str: String = r.get("payload");
            let created_at_str: String = r.get("created_at");
            let timestamp = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            if let Ok(event) = serde_json::from_str::<DomainEvent>(&payload_str) {
                events.push(EventEnvelope {
                    id: Uuid::new_v4().to_string(),
                    epoch: self.epoch.clone(),
                    sequence: seq as u64,
                    timestamp,
                    event,
                });
            }
        }

        Ok(ReplayOutcome::Events(events))
    }

    /// Vacuum old events beyond the retention period (default: 24 hours).
    pub async fn vacuum(&self, retain: Duration) -> anyhow::Result<u64> {
        let cutoff = Utc::now() - chrono::Duration::from_std(retain)?;
        let cutoff_str = cutoff.to_rfc3339();

        let res = sqlx::query("DELETE FROM event_journal WHERE created_at < ?")
            .bind(&cutoff_str)
            .execute(&self.db)
            .await?;

        Ok(res.rows_affected())
    }
}
