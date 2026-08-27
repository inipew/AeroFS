-- 0002_shares_and_trash.sql
-- Table for public/private shareable links
CREATE TABLE IF NOT EXISTS shares (
    id TEXT PRIMARY KEY NOT NULL,
    connection_id TEXT NOT NULL,
    path TEXT NOT NULL,
    share_token TEXT UNIQUE NOT NULL,
    password_hash TEXT,
    expires_at TEXT,
    created_at TEXT NOT NULL,
    created_by TEXT
);

CREATE INDEX IF NOT EXISTS idx_shares_token ON shares(share_token);

-- Table for Trash (Soft-delete & Restore)
CREATE TABLE IF NOT EXISTS trash_items (
    id TEXT PRIMARY KEY NOT NULL,
    connection_id TEXT NOT NULL,
    original_path TEXT NOT NULL,
    trash_path TEXT NOT NULL,
    item_name TEXT NOT NULL,
    is_directory INTEGER NOT NULL DEFAULT 0,
    size INTEGER,
    deleted_at TEXT NOT NULL,
    deleted_by TEXT
);

CREATE INDEX IF NOT EXISTS idx_trash_connection ON trash_items(connection_id);
