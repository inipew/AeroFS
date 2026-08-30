-- 0009_transfer_execution_mode.sql
-- Execution mode & staging as implementation detail of TransferEngine (Upload-as-Transfer)
ALTER TABLE transfer_jobs ADD COLUMN execution_mode TEXT NOT NULL DEFAULT 'inline';
ALTER TABLE transfer_jobs ADD COLUMN staging TEXT NOT NULL DEFAULT 'none';
