use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Row, Sqlite};

/// Persistent checkpoint of an active or interrupted file transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferCheckpoint {
    pub transfer_id: String,
    pub offset: u64,
    pub total: u64,
    pub staging_path: String,
    pub source_etag: Option<String>,
    pub source_version: Option<String>,
    pub checksum_so_far: Option<String>,
    pub updated_at: DateTime<Utc>,
}

impl TransferCheckpoint {
    /// Save or update checkpoint in SQLite.
    pub async fn save(&self, db: &Pool<Sqlite>) -> anyhow::Result<()> {
        let updated_at_str = self.updated_at.to_rfc3339();
        sqlx::query(
            "INSERT INTO transfer_checkpoints (transfer_id, offset, total, staging_path, source_etag, source_version, checksum_so_far, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(transfer_id) DO UPDATE SET
                offset = excluded.offset,
                total = excluded.total,
                staging_path = excluded.staging_path,
                source_etag = excluded.source_etag,
                source_version = excluded.source_version,
                checksum_so_far = excluded.checksum_so_far,
                updated_at = excluded.updated_at",
        )
        .bind(&self.transfer_id)
        .bind(self.offset as i64)
        .bind(self.total as i64)
        .bind(&self.staging_path)
        .bind(&self.source_etag)
        .bind(&self.source_version)
        .bind(&self.checksum_so_far)
        .bind(&updated_at_str)
        .execute(db)
        .await?;
        Ok(())
    }

    /// Load checkpoint for a transfer ID if available.
    pub async fn load(db: &Pool<Sqlite>, transfer_id: &str) -> anyhow::Result<Option<Self>> {
        let row = sqlx::query(
            "SELECT transfer_id, offset, total, staging_path, source_etag, source_version, checksum_so_far, updated_at
             FROM transfer_checkpoints WHERE transfer_id = ?",
        )
        .bind(transfer_id)
        .fetch_optional(db)
        .await?;

        if let Some(r) = row {
            let updated_at_str: String = r.get("updated_at");
            let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            Ok(Some(Self {
                transfer_id: r.get("transfer_id"),
                offset: r.get::<i64, _>("offset") as u64,
                total: r.get::<i64, _>("total") as u64,
                staging_path: r.get("staging_path"),
                source_etag: r.get("source_etag"),
                source_version: r.get("source_version"),
                checksum_so_far: r.get("checksum_so_far"),
                updated_at,
            }))
        } else {
            Ok(None)
        }
    }

    /// Delete checkpoint upon successful transfer completion or cancellation.
    pub async fn delete(db: &Pool<Sqlite>, transfer_id: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM transfer_checkpoints WHERE transfer_id = ?")
            .bind(transfer_id)
            .execute(db)
            .await?;
        Ok(())
    }
}
