-- 0003_durable_transfers.sql
-- Table for persistent, durable background transfer jobs
CREATE TABLE IF NOT EXISTS transfer_jobs (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    transfer_type TEXT NOT NULL,
    source_connection_id TEXT NOT NULL,
    source_path TEXT NOT NULL,
    destination_connection_id TEXT NOT NULL,
    destination_path TEXT NOT NULL,
    status TEXT NOT NULL,
    transferred_bytes INTEGER NOT NULL DEFAULT 0,
    total_bytes INTEGER NOT NULL DEFAULT 0,
    speed_bytes_per_sec INTEGER NOT NULL DEFAULT 0,
    eta_seconds INTEGER,
    checksum TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_transfers_status ON transfer_jobs(status);
CREATE INDEX IF NOT EXISTS idx_transfers_created_at ON transfer_jobs(created_at);
