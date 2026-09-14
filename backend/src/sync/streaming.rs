use crate::db::DbPool;
use crate::sync::models::{FileManifest, SyncOpKind, SyncOperation, SyncStrategy};
use crate::sync::scanner::{VfsScanner, SCAN_BATCH_SIZE};
use crate::vfs::FileSystem;
use sqlx::pool::PoolConnection;
use sqlx::{QueryBuilder, Row, Sqlite};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

const SOURCE_TABLE: &str = "temp_aerofs_sync_source";
const DEST_TABLE: &str = "temp_aerofs_sync_dest";
const RENAME_TABLE: &str = "temp_aerofs_sync_renames";
const OP_BATCH_SIZE: i64 = SCAN_BATCH_SIZE as i64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlanPhase {
    Matched,
    SourceOnly,
    DestOnly,
    Done,
}

/// DB-backed manifest plan with bounded in-memory operation batches.
///
/// Source and destination manifests are staged into connection-local SQLite TEMP tables as the
/// scanner emits batches. This keeps heap usage proportional to `SCAN_BATCH_SIZE` while retaining
/// global rename detection and exact destination-only reconciliation semantics.
pub struct StreamingSyncPlan {
    conn: PoolConnection<Sqlite>,
    strategy: SyncStrategy,
    phase: PlanPhase,
    cursor: String,
    total_operations: u64,
}

impl StreamingSyncPlan {
    #[allow(clippy::too_many_arguments)]
    pub async fn prepare(
        db: &DbPool,
        src_fs: Arc<dyn FileSystem>,
        source_connection_id: &str,
        source_path: &str,
        dst_fs: Arc<dyn FileSystem>,
        destination_connection_id: &str,
        destination_path: &str,
        strategy: SyncStrategy,
        cancel: &CancellationToken,
    ) -> anyhow::Result<Self> {
        let mut conn = db.acquire().await?;
        Self::reset_temp_tables(&mut conn).await?;

        Self::stage_side(
            &mut conn,
            SOURCE_TABLE,
            VfsScanner::scan_directory_batches(
                src_fs,
                source_connection_id.to_string(),
                source_path.to_string(),
                cancel.clone(),
            ),
            cancel,
        )
        .await?;

        Self::stage_side(
            &mut conn,
            DEST_TABLE,
            VfsScanner::scan_directory_batches(
                dst_fs,
                destination_connection_id.to_string(),
                destination_path.to_string(),
                cancel.clone(),
            ),
            cancel,
        )
        .await?;

        if cancel.is_cancelled() {
            return Ok(Self {
                conn,
                strategy,
                phase: PlanPhase::Done,
                cursor: String::new(),
                total_operations: 0,
            });
        }

        // Resolve rename ownership once in SQLite. UNIQUE(old_path) prevents two source entries
        // with the same fingerprint from trying to rename the same destination object.
        sqlx::query(&format!(
            r#"
            WITH candidates AS (
                SELECT s.path AS new_path,
                       (
                           SELECT d.path
                           FROM {dest} d
                           WHERE d.fingerprint = s.fingerprint
                             AND NOT EXISTS (
                                 SELECT 1 FROM {source} sx WHERE sx.path = d.path
                             )
                           ORDER BY d.path
                           LIMIT 1
                       ) AS old_path
                FROM {source} s
                WHERE NOT EXISTS (
                    SELECT 1 FROM {dest} same_path WHERE same_path.path = s.path
                )
            )
            INSERT OR IGNORE INTO {renames} (old_path, new_path)
            SELECT old_path, new_path
            FROM candidates
            WHERE old_path IS NOT NULL
            ORDER BY new_path
            "#,
            source = SOURCE_TABLE,
            dest = DEST_TABLE,
            renames = RENAME_TABLE,
        ))
        .execute(&mut *conn)
        .await?;

        let source_count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {SOURCE_TABLE}"))
            .fetch_one(&mut *conn)
            .await?;
        let dest_only_count: i64 = sqlx::query_scalar(&format!(
            r#"
            SELECT COUNT(*)
            FROM {dest} d
            WHERE NOT EXISTS (SELECT 1 FROM {source} s WHERE s.path = d.path)
              AND NOT EXISTS (SELECT 1 FROM {renames} r WHERE r.old_path = d.path)
            "#,
            source = SOURCE_TABLE,
            dest = DEST_TABLE,
            renames = RENAME_TABLE,
        ))
        .fetch_one(&mut *conn)
        .await?;

        Ok(Self {
            conn,
            strategy,
            phase: PlanPhase::Matched,
            cursor: String::new(),
            total_operations: (source_count + dest_only_count).max(0) as u64,
        })
    }

    pub fn total_operations(&self) -> u64 {
        self.total_operations
    }

    pub async fn next_batch(&mut self) -> anyhow::Result<Option<Vec<SyncOperation>>> {
        loop {
            let batch = match self.phase {
                PlanPhase::Matched => self.next_matched_batch().await?,
                PlanPhase::SourceOnly => self.next_source_only_batch().await?,
                PlanPhase::DestOnly => self.next_dest_only_batch().await?,
                PlanPhase::Done => return Ok(None),
            };

            if !batch.is_empty() {
                return Ok(Some(batch));
            }

            self.phase = match self.phase {
                PlanPhase::Matched => PlanPhase::SourceOnly,
                PlanPhase::SourceOnly => PlanPhase::DestOnly,
                PlanPhase::DestOnly | PlanPhase::Done => PlanPhase::Done,
            };
            self.cursor.clear();
        }
    }

    async fn reset_temp_tables(conn: &mut PoolConnection<Sqlite>) -> anyhow::Result<()> {
        for table in [SOURCE_TABLE, DEST_TABLE, RENAME_TABLE] {
            sqlx::query(&format!("DROP TABLE IF EXISTS {table}"))
                .execute(&mut **conn)
                .await?;
        }
        for table in [SOURCE_TABLE, DEST_TABLE] {
            sqlx::query(&format!(
                r#"
                CREATE TEMP TABLE {table} (
                    path TEXT PRIMARY KEY,
                    size INTEGER NOT NULL,
                    modified_at INTEGER,
                    content_hash TEXT,
                    etag TEXT,
                    fingerprint TEXT NOT NULL
                )
                "#
            ))
            .execute(&mut **conn)
            .await?;
            sqlx::query(&format!(
                "CREATE INDEX {table}_fingerprint_idx ON {table}(fingerprint)"
            ))
            .execute(&mut **conn)
            .await?;
        }
        sqlx::query(&format!(
            "CREATE TEMP TABLE {RENAME_TABLE} (old_path TEXT PRIMARY KEY, new_path TEXT UNIQUE NOT NULL)"
        ))
        .execute(&mut **conn)
        .await?;
        Ok(())
    }

    async fn stage_side(
        conn: &mut PoolConnection<Sqlite>,
        table: &str,
        mut batches: tokio::sync::mpsc::Receiver<anyhow::Result<Vec<FileManifest>>>,
        cancel: &CancellationToken,
    ) -> anyhow::Result<()> {
        while let Some(batch) = batches.recv().await {
            if cancel.is_cancelled() {
                return Ok(());
            }
            let batch = batch?;
            if batch.is_empty() {
                continue;
            }

            let mut builder = QueryBuilder::<Sqlite>::new(format!(
                "INSERT OR REPLACE INTO {table} (path, size, modified_at, content_hash, etag, fingerprint) "
            ));
            builder.push_values(batch.iter(), |mut row, manifest| {
                row.push_bind(&manifest.path)
                    .push_bind(manifest.size as i64)
                    .push_bind(manifest.modified_at.map(|t| t.timestamp()))
                    .push_bind(&manifest.content_hash)
                    .push_bind(&manifest.etag)
                    .push_bind(Self::fingerprint(manifest));
            });
            builder.build().execute(&mut **conn).await?;
        }
        Ok(())
    }

    async fn next_matched_batch(&mut self) -> anyhow::Result<Vec<SyncOperation>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT s.path,
                   s.size AS source_size, s.modified_at AS source_modified,
                   s.content_hash AS source_hash, s.etag AS source_etag,
                   d.size AS dest_size, d.modified_at AS dest_modified,
                   d.content_hash AS dest_hash, d.etag AS dest_etag
            FROM {source} s
            JOIN {dest} d ON d.path = s.path
            WHERE s.path > ?
            ORDER BY s.path
            LIMIT ?
            "#,
            source = SOURCE_TABLE,
            dest = DEST_TABLE,
        ))
        .bind(&self.cursor)
        .bind(OP_BATCH_SIZE)
        .fetch_all(&mut *self.conn)
        .await?;

        let mut ops = Vec::with_capacity(rows.len());
        for row in rows {
            let path: String = row.get("path");
            self.cursor = path.clone();
            let source_size = row.get::<i64, _>("source_size") as u64;
            let dest_size = row.get::<i64, _>("dest_size") as u64;
            let source_modified: Option<i64> = row.get("source_modified");
            let dest_modified: Option<i64> = row.get("dest_modified");
            let source_hash: Option<String> = row.get("source_hash");
            let dest_hash: Option<String> = row.get("dest_hash");
            let source_etag: Option<String> = row.get("source_etag");
            let dest_etag: Option<String> = row.get("dest_etag");

            let same_size = source_size == dest_size;
            let same_hash = matches!((&source_hash, &dest_hash), (Some(a), Some(b)) if a == b);
            let same_etag = matches!((&source_etag, &dest_etag), (Some(a), Some(b)) if a == b);
            let unchanged = same_hash
                || (same_etag && same_size)
                || (same_size && source_modified == dest_modified);

            let kind = if unchanged {
                SyncOpKind::Noop
            } else {
                match self.strategy {
                    SyncStrategy::SourceWins => SyncOpKind::Update,
                    SyncStrategy::DestWins => SyncOpKind::Noop,
                    SyncStrategy::NewestWins => {
                        if source_modified >= dest_modified {
                            SyncOpKind::Update
                        } else {
                            SyncOpKind::Noop
                        }
                    }
                    SyncStrategy::KeepBoth | SyncStrategy::Manual => SyncOpKind::Conflict,
                }
            };
            ops.push(SyncOperation {
                relative_path: path,
                kind,
                source_manifest: None,
                dest_manifest: None,
            });
        }
        Ok(ops)
    }

    async fn next_source_only_batch(&mut self) -> anyhow::Result<Vec<SyncOperation>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT s.path, r.old_path
            FROM {source} s
            LEFT JOIN {dest} d ON d.path = s.path
            LEFT JOIN {renames} r ON r.new_path = s.path
            WHERE d.path IS NULL AND s.path > ?
            ORDER BY s.path
            LIMIT ?
            "#,
            source = SOURCE_TABLE,
            dest = DEST_TABLE,
            renames = RENAME_TABLE,
        ))
        .bind(&self.cursor)
        .bind(OP_BATCH_SIZE)
        .fetch_all(&mut *self.conn)
        .await?;

        let mut ops = Vec::with_capacity(rows.len());
        for row in rows {
            let path: String = row.get("path");
            let old_path: Option<String> = row.get("old_path");
            self.cursor = path.clone();
            ops.push(SyncOperation {
                relative_path: path,
                kind: old_path
                    .map(|old_path| SyncOpKind::Rename { old_path })
                    .unwrap_or(SyncOpKind::Create),
                source_manifest: None,
                dest_manifest: None,
            });
        }
        Ok(ops)
    }

    async fn next_dest_only_batch(&mut self) -> anyhow::Result<Vec<SyncOperation>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT d.path
            FROM {dest} d
            WHERE d.path > ?
              AND NOT EXISTS (SELECT 1 FROM {source} s WHERE s.path = d.path)
              AND NOT EXISTS (SELECT 1 FROM {renames} r WHERE r.old_path = d.path)
            ORDER BY d.path
            LIMIT ?
            "#,
            source = SOURCE_TABLE,
            dest = DEST_TABLE,
            renames = RENAME_TABLE,
        ))
        .bind(&self.cursor)
        .bind(OP_BATCH_SIZE)
        .fetch_all(&mut *self.conn)
        .await?;

        let mut ops = Vec::with_capacity(rows.len());
        for row in rows {
            let path: String = row.get("path");
            self.cursor = path.clone();
            let kind = match self.strategy {
                SyncStrategy::SourceWins | SyncStrategy::NewestWins => SyncOpKind::Delete,
                SyncStrategy::DestWins | SyncStrategy::KeepBoth => SyncOpKind::Noop,
                SyncStrategy::Manual => SyncOpKind::Conflict,
            };
            ops.push(SyncOperation {
                relative_path: path,
                kind,
                source_manifest: None,
                dest_manifest: None,
            });
        }
        Ok(ops)
    }

    fn fingerprint(manifest: &FileManifest) -> String {
        if let Some(etag) = &manifest.etag {
            format!("etag:{etag}")
        } else if let Some(hash) = &manifest.content_hash {
            format!("hash:{hash}")
        } else if let Some(modified_at) = manifest.modified_at {
            format!("size_time:{}_{}", manifest.size, modified_at.timestamp())
        } else {
            format!("size:{}", manifest.size)
        }
    }
}
