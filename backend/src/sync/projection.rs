use super::SyncManager;
use sqlx::Row;

impl SyncManager {
    /// Apply one transfer-completion projection idempotently.
    ///
    /// EventJournal consumers are at-least-once: a process may crash after applying
    /// the projection but before committing its consumer cursor. Terminal operations
    /// therefore must be recognized as already applied so synced counters are not
    /// incremented twice during replay.
    pub(crate) async fn apply_transfer_completion_event(
        &self,
        transfer_job_id: &str,
        success: bool,
    ) -> anyhow::Result<()> {
        let op = sqlx::query(
            "SELECT id, job_id, status FROM sync_operations WHERE transfer_job_id = ?",
        )
        .bind(transfer_job_id)
        .fetch_optional(&self.db)
        .await?;

        let Some(row) = op else {
            return Ok(());
        };

        let op_id: String = row.get("id");
        let job_id: String = row.get("job_id");
        let current_status: String = row.get("status");
        let target_status = if success { "completed" } else { "failed" };

        // A replay of the same terminal outcome is a no-op. If an operation already
        // reached another terminal outcome, preserve the first terminal decision and
        // rely on explicit retry/reconciliation to change it.
        if matches!(current_status.as_str(), "completed" | "failed") {
            return Ok(());
        }

        let err = (!success).then_some("Transfer failed");
        self.update_operation_status(&op_id, target_status, Some(transfer_job_id), err)
            .await?;

        if success {
            self.increment_synced(&job_id).await?;
        }

        self.check_job_completion(&job_id).await?;
        Ok(())
    }
}
