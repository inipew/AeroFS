-- Cancellation is terminal once it has been durably observed.
--
-- Two writers participate in cancellation today: the request path persists
-- `cancellation_requested`, while the worker persists `cancelled` after cleanup.
-- The worker can finish between the request path signalling its token and writing
-- its snapshot, allowing the stale request snapshot to overwrite `cancelled`.
--
-- Keep the durable state machine monotonic and make the durable terminal event an
-- atomic last line of defence: EventJournal broadcasts only after this INSERT
-- (and therefore these trigger effects) succeeds.

CREATE TRIGGER IF NOT EXISTS trg_transfer_cancelled_event_projects_terminal_state
AFTER INSERT ON event_journal
WHEN NEW.event_type = 'transfer_cancelled' AND NEW.aggregate_id IS NOT NULL
BEGIN
    UPDATE transfer_jobs
    SET status = 'cancelled',
        speed_bytes_per_sec = 0,
        eta_seconds = NULL,
        updated_at = NEW.created_at
    WHERE id = NEW.aggregate_id
      AND status IN ('queued', 'running', 'cancellation_requested', 'cancelled');
END;

CREATE TRIGGER IF NOT EXISTS trg_transfer_cancelled_state_is_terminal
BEFORE UPDATE OF status ON transfer_jobs
WHEN OLD.status = 'cancelled' AND NEW.status <> 'cancelled'
BEGIN
    SELECT RAISE(IGNORE);
END;
