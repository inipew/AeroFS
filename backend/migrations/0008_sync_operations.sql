-- 0008_sync_operations.sql
-- Persistent sync operations tracking each file synchronization lifecycle

CREATE TABLE IF NOT EXISTS sync_operations (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL,
    op_kind TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    old_path TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    transfer_job_id TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(job_id) REFERENCES sync_jobs(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_sync_operations_job_id ON sync_operations(job_id);
CREATE INDEX IF NOT EXISTS idx_sync_operations_transfer_id ON sync_operations(transfer_job_id);
