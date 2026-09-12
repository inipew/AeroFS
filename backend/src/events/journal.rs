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
    #[serde(rename = "transfer_cancelled")]
    TransferCancelled(serde_json::Value),
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
    FullSync { reason: String, epoch: String },
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

    pub fn transfer_progress(job: &crate::transfer::TransferJob) -> Self {
        DomainEvent::TransferProgress(serde_json::to_value(job.to_response()).unwrap_or_default())
    }

    pub fn transfer_completed(job: &crate::transfer::TransferJob) -> Self {
        DomainEvent::TransferCompleted(serde_json::to_value(job.to_response()).unwrap_or_default())
    }

    pub fn transfer_failed(job: &crate::transfer::TransferJob) -> Self {
        DomainEvent::TransferFailed(serde_json::to_value(job.to_response()).unwrap_or_default())
    }

    pub fn transfer_cancelled(job: &crate::transfer::TransferJob) -> Self {
        DomainEvent::TransferCancelled(serde_json::to_value(job.to_response()).unwrap_or_default())
    }

    pub fn event_type_name(&self) -> &'static str {
        match self {
            DomainEvent::TransferProgress(_) => "transfer_progress",
            DomainEvent::TransferCompleted(_) => "transfer_completed",
            DomainEvent::TransferFailed(_) => "transfer_failed",
            DomainEvent::TransferCancelled(_) => "transfer_cancelled",
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
    #[serde(skip)]
    pub journal_id: Option<i64>,
    #[serde(flatten)]
    pub event: DomainEvent,
}

#[derive(Debug, Clone)]
pub enum ReplayOutcome {
    Events(Vec<EventEnvelope>),
    Expired {
        latest_sequence: u64,
    },
    EpochMismatch {
        current_epoch: String,
        latest_sequence: u64,
    },
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
    /// Initialize EventJournal with a unique generation epoch.
    pub async fn init(db: Pool<Sqlite>) -> anyhow::Result<Self> {
        let epoch = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO server_epoch (id, epoch, created_at) VALUES (1, ?, ?)\n             ON CONFLICT(id) DO UPDATE SET epoch = excluded.epoch, created_at = excluded.created_at",
        )
        .bind(&epoch)
        .bind(&now)
        .execute(&db)
        .await?;

        let (event_tx, _) = broadcast::channel::<EventEnvelope>(1000);

        Ok(Self {
            db,
            epoch,
            sequence_counter: Arc::new(AtomicU64::new(0)),
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

    /// Append an event to durable storage before publishing it to live subscribers.
    /// Progress ticks intentionally stay transient, while lifecycle/file events receive
    /// a global journal row id usable by durable internal consumers across server epochs.
    pub async fn append(
        &self,
        event: DomainEvent,
        aggregate_id: Option<&str>,
    ) -> anyhow::Result<EventEnvelope> {
        let seq = self.sequence_counter.fetch_add(1, Ordering::SeqCst) + 1;
        let event_id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let is_progress = matches!(event, DomainEvent::TransferProgress(_));

        let journal_id = if is_progress {
            None
        } else {
            let payload = serde_json::to_string(&event)?;
            let event_type = event.event_type_name();
            let result = sqlx::query(
                "INSERT INTO event_journal (epoch, sequence, event_type, aggregate_id, payload, created_at)\n                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(&self.epoch)
            .bind(seq as i64)
            .bind(event_type)
            .bind(aggregate_id)
            .bind(payload)
            .bind(&now_str)
            .execute(&self.db)
            .await?;
            Some(result.last_insert_rowid())
        };

        let envelope = EventEnvelope {
            id: event_id,
            epoch: self.epoch.clone(),
            sequence: seq,
            timestamp: now,
            journal_id,
            event,
        };

        // Publish only after persistence succeeds so durable consumers never observe
        // a lifecycle event that cannot subsequently be replayed.
        let _ = self.event_tx.send(envelope.clone());
        Ok(envelope)
    }

    /// Read durable journal rows globally by SQLite row id. Unlike websocket replay,
    /// this cursor intentionally spans server epochs and is meant for internal consumers.
    pub async fn durable_events_after(
        &self,
        after_id: i64,
        limit: usize,
    ) -> anyhow::Result<Vec<EventEnvelope>> {
        let rows = sqlx::query(
            "SELECT id, epoch, sequence, payload, created_at FROM event_journal\n             WHERE id > ? ORDER BY id ASC LIMIT ?",
        )
        .bind(after_id)
        .bind(limit as i64)
        .fetch_all(&self.db)
        .await?;

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            let journal_id: i64 = row.get("id");
            let epoch: String = row.get("epoch");
            let sequence: i64 = row.get("sequence");
            let payload: String = row.get("payload");
            let created_at: String = row.get("created_at");
            let timestamp = DateTime::parse_from_rfc3339(&created_at)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            if let Ok(event) = serde_json::from_str::<DomainEvent>(&payload) {
                events.push(EventEnvelope {
                    id: format!("journal-{journal_id}"),
                    epoch,
                    sequence: sequence as u64,
                    timestamp,
                    journal_id: Some(journal_id),
                    event,
                });
            }
        }
        Ok(events)
    }

    /// Loads and durably registers an internal consumer at cursor zero when first seen.
    /// Registration is important because vacuum must not delete backlog that a known
    /// projection has not processed yet.
    pub async fn consumer_cursor(&self, consumer: &str) -> anyhow::Result<i64> {
        sqlx::query(
            "INSERT INTO event_consumer_cursors (consumer, last_event_id, updated_at)\n             VALUES (?, 0, ?) ON CONFLICT(consumer) DO NOTHING",
        )
        .bind(consumer)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.db)
        .await?;

        let value: i64 = sqlx::query_scalar(
            "SELECT last_event_id FROM event_consumer_cursors WHERE consumer = ?",
        )
        .bind(consumer)
        .fetch_one(&self.db)
        .await?;
        Ok(value)
    }

    pub async fn store_consumer_cursor(
        &self,
        consumer: &str,
        last_event_id: i64,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO event_consumer_cursors (consumer, last_event_id, updated_at)\n             VALUES (?, ?, ?)\n             ON CONFLICT(consumer) DO UPDATE SET\n               last_event_id = CASE\n                 WHEN excluded.last_event_id > event_consumer_cursors.last_event_id\n                 THEN excluded.last_event_id ELSE event_consumer_cursors.last_event_id END,\n               updated_at = excluded.updated_at",
        )
        .bind(consumer)
        .bind(last_event_id)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.db)
        .await?;
        Ok(())
    }

    /// Replay missed websocket events within the current server epoch.
    pub async fn get_since(
        &self,
        client_epoch: Option<&str>,
        last_sequence: u64,
        limit: usize,
    ) -> anyhow::Result<ReplayOutcome> {
        let current_latest = self.latest_sequence();

        if let Some(ep) = client_epoch {
            if ep != self.epoch {
                return Ok(ReplayOutcome::EpochMismatch {
                    current_epoch: self.epoch.clone(),
                    latest_sequence: current_latest,
                });
            }
        } else {
            return Ok(ReplayOutcome::EpochMismatch {
                current_epoch: self.epoch.clone(),
                latest_sequence: current_latest,
            });
        }

        if last_sequence >= current_latest {
            return Ok(ReplayOutcome::Events(Vec::new()));
        }

        let min_sequence: Option<i64> =
            sqlx::query_scalar("SELECT MIN(sequence) FROM event_journal WHERE epoch = ?")
                .bind(&self.epoch)
                .fetch_one(&self.db)
                .await?;

        match min_sequence {
            Some(min_seq) => {
                if (last_sequence + 1) < min_seq as u64 {
                    return Ok(ReplayOutcome::Expired {
                        latest_sequence: current_latest,
                    });
                }
            }
            None => {
                if last_sequence + 1 < current_latest {
                    return Ok(ReplayOutcome::Expired {
                        latest_sequence: current_latest,
                    });
                }
            }
        }

        let rows = sqlx::query(
            "SELECT id, sequence, payload, created_at FROM event_journal\n             WHERE epoch = ? AND sequence > ?\n             ORDER BY sequence ASC LIMIT ?",
        )
        .bind(&self.epoch)
        .bind(last_sequence as i64)
        .bind(limit as i64)
        .fetch_all(&self.db)
        .await?;

        // Sequence gaps can legitimately be transient transfer_progress events, which
        // are not persisted. Do not interpret such gaps as journal expiration.
        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            let journal_id: i64 = row.get("id");
            let sequence: i64 = row.get("sequence");
            let payload: String = row.get("payload");
            let created_at: String = row.get("created_at");
            let timestamp = DateTime::parse_from_rfc3339(&created_at)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            if let Ok(event) = serde_json::from_str::<DomainEvent>(&payload) {
                events.push(EventEnvelope {
                    id: format!("journal-{journal_id}"),
                    epoch: self.epoch.clone(),
                    sequence: sequence as u64,
                    timestamp,
                    journal_id: Some(journal_id),
                    event,
                });
            }
        }

        Ok(ReplayOutcome::Events(events))
    }

    /// Vacuum events older than the retention period without crossing the slowest
    /// durable consumer. Rows at or below a consumer cursor have already been applied;
    /// rows above the minimum cursor are retained even when their age exceeds retention.
    pub async fn vacuum(&self, retain: Duration) -> anyhow::Result<u64> {
        let cutoff = Utc::now() - chrono::Duration::from_std(retain)?;
        let cutoff_str = cutoff.to_rfc3339();
        let min_cursor: Option<i64> =
            sqlx::query_scalar("SELECT MIN(last_event_id) FROM event_consumer_cursors")
                .fetch_one(&self.db)
                .await?;

        let res = if let Some(min_cursor) = min_cursor {
            sqlx::query("DELETE FROM event_journal WHERE created_at < ? AND id <= ?")
                .bind(&cutoff_str)
                .bind(min_cursor)
                .execute(&self.db)
                .await?
        } else {
            sqlx::query("DELETE FROM event_journal WHERE created_at < ?")
                .bind(&cutoff_str)
                .execute(&self.db)
                .await?
        };

        Ok(res.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::DomainEvent;

    #[test]
    fn transfer_cancelled_has_a_distinct_wire_type() {
        let event = DomainEvent::TransferCancelled(serde_json::json!({ "id": "job-1" }));
        assert_eq!(event.event_type_name(), "transfer_cancelled");
        assert_eq!(
            serde_json::to_value(event).unwrap()["type"],
            "transfer_cancelled"
        );
    }
}
