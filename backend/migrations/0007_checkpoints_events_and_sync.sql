-- 0007_checkpoints_events_and_sync.sql
-- Checkpoint persistence for resume, event journal durability with epoch, and sync tables

CREATE TABLE IF NOT EXISTS transfer_checkpoints (
    transfer_id TEXT PRIMARY KEY,
    offset INTEGER NOT NULL DEFAULT 0,
    total INTEGER NOT NULL DEFAULT 0,
    staging_path TEXT NOT NULL,
    source_etag TEXT,
    source_version TEXT,
    checksum_so_far TEXT,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS server_epoch (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    epoch TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS event_journal (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    epoch TEXT NOT NULL,
    sequence INTEGER NOT NULL,
    event_type TEXT NOT NULL,
    aggregate_id TEXT,
    payload TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_event_journal_epoch_seq ON event_journal(epoch, sequence);

CREATE TABLE IF NOT EXISTS sync_jobs (
    id TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL,
    source_connection_id TEXT NOT NULL,
    source_path TEXT NOT NULL,
    destination_connection_id TEXT NOT NULL,
    destination_path TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'created',
    strategy TEXT NOT NULL DEFAULT 'keep_both',
    total_files INTEGER NOT NULL DEFAULT 0,
    synced_files INTEGER NOT NULL DEFAULT 0,
    conflict_files INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
