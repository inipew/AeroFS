-- 0011_sync_history_pagination.sql
-- Composite indexes for bounded keyset pagination of sync job and operation history.

CREATE INDEX IF NOT EXISTS idx_sync_jobs_created_id
    ON sync_jobs(created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_sync_operations_job_created_id
    ON sync_operations(job_id, created_at ASC, id ASC);
