-- 0006_transfer_phase.sql
-- Add explicit transfer phase tracking to prevent premature 100% completion illusion
ALTER TABLE transfer_jobs ADD COLUMN phase TEXT NOT NULL DEFAULT 'preparing';
CREATE INDEX IF NOT EXISTS idx_transfers_phase ON transfer_jobs(phase);
