-- 0004_transfer_ownership_and_dismiss.sql
-- Add user ownership and dismissed_at timestamp to transfer_jobs
ALTER TABLE transfer_jobs ADD COLUMN user_id TEXT;
ALTER TABLE transfer_jobs ADD COLUMN dismissed_at TEXT;

CREATE INDEX IF NOT EXISTS idx_transfers_user_dismissed ON transfer_jobs(user_id, dismissed_at);
