-- 0012_runtime_execution_metrics.sql
-- Keep aggregate runtime lifecycle metrics bounded by indexed status lookups.

CREATE INDEX IF NOT EXISTS idx_sync_jobs_status
    ON sync_jobs(status);
