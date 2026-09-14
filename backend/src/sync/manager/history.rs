use super::{SyncManager, SyncOperationRow};
use crate::sync::models::SyncJob;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::DateTime;
use sqlx::Row;

pub const SYNC_HISTORY_DEFAULT_LIMIT: usize = 100;
pub const SYNC_HISTORY_MAX_LIMIT: usize = 500;

const JOB_CURSOR_SCOPE: &str = "sync-jobs";
const OPERATION_CURSOR_SCOPE: &str = "sync-operations";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncPageCursor {
    scope: String,
    created_at: String,
    id: String,
}

impl SyncPageCursor {
    fn new(scope: &str, created_at: String, id: String) -> Self {
        Self {
            scope: scope.to_string(),
            created_at,
            id,
        }
    }

    fn jobs(created_at: String, id: String) -> Self {
        Self::new(JOB_CURSOR_SCOPE, created_at, id)
    }

    fn operations(created_at: String, id: String) -> Self {
        Self::new(OPERATION_CURSOR_SCOPE, created_at, id)
    }

    pub fn encode(&self) -> String {
        URL_SAFE_NO_PAD.encode(format!("{}\n{}\n{}", self.scope, self.created_at, self.id))
    }

    fn decode_for(value: &str, expected_scope: &str) -> Result<Self, String> {
        let decoded = URL_SAFE_NO_PAD
            .decode(value)
            .map_err(|_| "Invalid sync pagination cursor".to_string())?;
        let decoded = String::from_utf8(decoded)
            .map_err(|_| "Invalid sync pagination cursor".to_string())?;
        let mut parts = decoded.splitn(3, '\n');
        let scope = parts.next().unwrap_or_default();
        let created_at = parts.next().unwrap_or_default();
        let id = parts.next().unwrap_or_default();

        if scope != expected_scope || created_at.is_empty() || id.is_empty() {
            return Err("Invalid sync pagination cursor".to_string());
        }
        DateTime::parse_from_rfc3339(created_at)
            .map_err(|_| "Invalid sync pagination cursor".to_string())?;

        Ok(Self::new(scope, created_at.to_string(), id.to_string()))
    }

    pub fn decode_jobs(value: &str) -> Result<Self, String> {
        Self::decode_for(value, JOB_CURSOR_SCOPE)
    }

    pub fn decode_operations(value: &str) -> Result<Self, String> {
        Self::decode_for(value, OPERATION_CURSOR_SCOPE)
    }
}

#[derive(Debug, Clone)]
pub struct SyncHistoryPage<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<SyncPageCursor>,
}

fn bounded_limit(limit: Option<usize>) -> usize {
    limit
        .unwrap_or(SYNC_HISTORY_DEFAULT_LIMIT)
        .clamp(1, SYNC_HISTORY_MAX_LIMIT)
}

impl SyncManager {
    pub async fn list_jobs_page(
        &self,
        cursor: Option<&SyncPageCursor>,
        limit: Option<usize>,
    ) -> anyhow::Result<SyncHistoryPage<SyncJob>> {
        let limit = bounded_limit(limit);
        let fetch_limit = limit.saturating_add(1) as i64;
        let rows = if let Some(cursor) = cursor {
            sqlx::query(
                "SELECT id, user_id, source_connection_id, source_path, destination_connection_id, \
                 destination_path, status, strategy, total_files, synced_files, conflict_files, \
                 created_at, updated_at FROM sync_jobs \
                 WHERE (created_at, id) < (?, ?) \
                 ORDER BY created_at DESC, id DESC LIMIT ?",
            )
            .bind(&cursor.created_at)
            .bind(&cursor.id)
            .bind(fetch_limit)
            .fetch_all(&self.db)
            .await?
        } else {
            sqlx::query(
                "SELECT id, user_id, source_connection_id, source_path, destination_connection_id, \
                 destination_path, status, strategy, total_files, synced_files, conflict_files, \
                 created_at, updated_at FROM sync_jobs \
                 ORDER BY created_at DESC, id DESC LIMIT ?",
            )
            .bind(fetch_limit)
            .fetch_all(&self.db)
            .await?
        };

        let has_more = rows.len() > limit;
        let mut items: Vec<SyncJob> = rows
            .iter()
            .take(limit)
            .map(Self::sync_job_from_row)
            .collect();
        let next_cursor = if has_more {
            items.last().map(|job| {
                SyncPageCursor::jobs(job.created_at.to_rfc3339(), job.id.clone())
            })
        } else {
            None
        };

        // `sync_jobs` is persisted before in-memory transitions are updated, so SQLite is the
        // authoritative and bounded history source. Do not merge the entire active map here: that
        // would defeat keyset ordering and is unnecessary for visibility correctness.
        Ok(SyncHistoryPage {
            items: std::mem::take(&mut items),
            next_cursor,
        })
    }

    pub async fn list_operations_page(
        &self,
        job_id: &str,
        cursor: Option<&SyncPageCursor>,
        limit: Option<usize>,
    ) -> anyhow::Result<SyncHistoryPage<SyncOperationRow>> {
        let limit = bounded_limit(limit);
        let fetch_limit = limit.saturating_add(1) as i64;
        let rows = if let Some(cursor) = cursor {
            sqlx::query(
                "SELECT id, job_id, op_kind, relative_path, old_path, status, transfer_job_id, \
                 error_message, created_at, updated_at FROM sync_operations \
                 WHERE job_id = ? AND (created_at, id) > (?, ?) \
                 ORDER BY created_at ASC, id ASC LIMIT ?",
            )
            .bind(job_id)
            .bind(&cursor.created_at)
            .bind(&cursor.id)
            .bind(fetch_limit)
            .fetch_all(&self.db)
            .await?
        } else {
            sqlx::query(
                "SELECT id, job_id, op_kind, relative_path, old_path, status, transfer_job_id, \
                 error_message, created_at, updated_at FROM sync_operations \
                 WHERE job_id = ? ORDER BY created_at ASC, id ASC LIMIT ?",
            )
            .bind(job_id)
            .bind(fetch_limit)
            .fetch_all(&self.db)
            .await?
        };

        let has_more = rows.len() > limit;
        let items: Vec<SyncOperationRow> = rows
            .into_iter()
            .take(limit)
            .map(|row| SyncOperationRow {
                id: row.get("id"),
                job_id: row.get("job_id"),
                op_kind: row.get("op_kind"),
                relative_path: row.get("relative_path"),
                old_path: row.get("old_path"),
                status: row.get("status"),
                transfer_job_id: row.get("transfer_job_id"),
                error_message: row.get("error_message"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect();
        let next_cursor = if has_more {
            items.last().map(|operation| {
                SyncPageCursor::operations(operation.created_at.clone(), operation.id.clone())
            })
        } else {
            None
        };

        Ok(SyncHistoryPage { items, next_cursor })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn jobs_db() -> crate::db::DbPool {
        let db = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE sync_jobs (\
                id TEXT PRIMARY KEY, user_id TEXT NOT NULL, source_connection_id TEXT NOT NULL, \
                source_path TEXT NOT NULL, destination_connection_id TEXT NOT NULL, \
                destination_path TEXT NOT NULL, status TEXT NOT NULL, strategy TEXT NOT NULL, \
                total_files INTEGER NOT NULL, synced_files INTEGER NOT NULL, \
                conflict_files INTEGER NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL\
            )",
        )
        .execute(&db)
        .await
        .unwrap();
        db
    }

    async fn operations_db() -> crate::db::DbPool {
        let db = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE sync_operations (\
                id TEXT PRIMARY KEY, job_id TEXT NOT NULL, op_kind TEXT NOT NULL, \
                relative_path TEXT NOT NULL, old_path TEXT, status TEXT NOT NULL, \
                transfer_job_id TEXT, error_message TEXT, created_at TEXT NOT NULL, \
                updated_at TEXT NOT NULL\
            )",
        )
        .execute(&db)
        .await
        .unwrap();
        db
    }

    #[test]
    fn cursor_is_opaque_scoped_and_validated() {
        let cursor = SyncPageCursor::jobs(
            "2026-09-14T08:00:00+00:00".into(),
            "job-1".into(),
        );
        let encoded = cursor.encode();
        assert_eq!(SyncPageCursor::decode_jobs(&encoded).unwrap(), cursor);
        assert!(SyncPageCursor::decode_operations(&encoded).is_err());
        assert!(SyncPageCursor::decode_jobs("not-base64").is_err());
    }

    #[test]
    fn page_limit_is_always_bounded() {
        assert_eq!(bounded_limit(None), SYNC_HISTORY_DEFAULT_LIMIT);
        assert_eq!(bounded_limit(Some(0)), 1);
        assert_eq!(bounded_limit(Some(10)), 10);
        assert_eq!(bounded_limit(Some(usize::MAX)), SYNC_HISTORY_MAX_LIMIT);
    }

    #[tokio::test]
    async fn job_history_keyset_pages_without_materializing_all_rows() {
        let db = jobs_db().await;
        for index in 0..205usize {
            sqlx::query(
                "INSERT INTO sync_jobs (\
                    id, user_id, source_connection_id, source_path, destination_connection_id, \
                    destination_path, status, strategy, total_files, synced_files, conflict_files, \
                    created_at, updated_at\
                 ) VALUES (?, 'u', 'local', '/src', 'remote', '/dst', 'completed', 'source_wins', \
                           1, 1, 0, ?, ?)",
            )
            .bind(format!("job-{index:04}"))
            .bind(format!("2026-09-14T08:{:02}:{:02}+00:00", (index / 60) % 60, index % 60))
            .bind("2026-09-14T09:00:00+00:00")
            .execute(&db)
            .await
            .unwrap();
        }

        let rows = sqlx::query("SELECT COUNT(*) AS count FROM sync_jobs")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(rows.get::<i64, _>("count"), 205);

        let first_rows = sqlx::query(
            "SELECT id, created_at FROM sync_jobs ORDER BY created_at DESC, id DESC LIMIT 101",
        )
        .fetch_all(&db)
        .await
        .unwrap();
        assert_eq!(first_rows.len(), 101);

        let boundary_created: String = first_rows[99].get("created_at");
        let boundary_id: String = first_rows[99].get("id");
        let second_rows = sqlx::query(
            "SELECT id FROM sync_jobs WHERE (created_at, id) < (?, ?) \
             ORDER BY created_at DESC, id DESC LIMIT 101",
        )
        .bind(boundary_created)
        .bind(boundary_id)
        .fetch_all(&db)
        .await
        .unwrap();
        assert_eq!(second_rows.len(), 101);
    }

    #[tokio::test]
    async fn operation_history_keyset_is_scoped_to_one_job() {
        let db = operations_db().await;
        for index in 0..25usize {
            for job in ["job-a", "job-b"] {
                sqlx::query(
                    "INSERT INTO sync_operations (\
                        id, job_id, op_kind, relative_path, old_path, status, transfer_job_id, \
                        error_message, created_at, updated_at\
                     ) VALUES (?, ?, 'noop', ?, NULL, 'completed', NULL, NULL, ?, ?)",
                )
                .bind(format!("{job}-op-{index:04}"))
                .bind(job)
                .bind(format!("file-{index:04}"))
                .bind(format!("2026-09-14T08:00:{index:02}+00:00"))
                .bind("2026-09-14T09:00:00+00:00")
                .execute(&db)
                .await
                .unwrap();
            }
        }

        let first = sqlx::query(
            "SELECT id, created_at FROM sync_operations WHERE job_id = 'job-a' \
             ORDER BY created_at ASC, id ASC LIMIT 11",
        )
        .fetch_all(&db)
        .await
        .unwrap();
        assert_eq!(first.len(), 11);
        let boundary_created: String = first[9].get("created_at");
        let boundary_id: String = first[9].get("id");
        let second = sqlx::query(
            "SELECT job_id FROM sync_operations WHERE job_id = 'job-a' \
             AND (created_at, id) > (?, ?) ORDER BY created_at ASC, id ASC LIMIT 11",
        )
        .bind(boundary_created)
        .bind(boundary_id)
        .fetch_all(&db)
        .await
        .unwrap();
        assert_eq!(second.len(), 11);
        assert!(second
            .iter()
            .all(|row| row.get::<String, _>("job_id") == "job-a"));
    }
}
