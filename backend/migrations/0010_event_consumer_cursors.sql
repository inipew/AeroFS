-- 0010_event_consumer_cursors.sql
-- Durable projection cursors for internal event-journal consumers.

CREATE TABLE IF NOT EXISTS event_consumer_cursors (
    consumer TEXT PRIMARY KEY,
    last_event_id INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL
);
