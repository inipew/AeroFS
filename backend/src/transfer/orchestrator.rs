use crate::transfer::{
    RetryTransferError, TransferJob, TransferManager, TransferType,
};

/// Canonical command admitted by the transfer subsystem. HTTP, Sync and future
/// schedulers should submit this command rather than call TransferManager directly.
#[derive(Debug, Clone)]
pub struct TransferCommand {
    pub user_id: Option<String>,
    pub name: String,
    pub transfer_type: TransferType,
    pub source_connection_id: String,
    pub source_path: String,
    pub destination_connection_id: String,
    pub destination_path: String,
}

impl TransferCommand {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("Transfer name must not be empty".into());
        }
        if self.source_connection_id.trim().is_empty()
            || self.destination_connection_id.trim().is_empty()
        {
            return Err("Transfer connection id must not be empty".into());
        }
        if self.source_path.trim().is_empty() || self.destination_path.trim().is_empty() {
            return Err("Transfer path must not be empty".into());
        }
        if self.source_connection_id == self.destination_connection_id
            && self.source_path == self.destination_path
        {
            return Err("Source and destination must be different".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferAdmission {
    pub job_id: String,
}

/// Stable orchestration boundary for the transfer subsystem.
///
/// `TransferManager` remains the scheduler/persistence implementation for now;
/// callers depend on this facade so its internals can be split without another
/// API/application migration.
#[derive(Clone)]
pub struct TransferEngine {
    manager: TransferManager,
}

impl TransferEngine {
    pub fn new(manager: TransferManager) -> Self {
        Self { manager }
    }

    pub async fn submit(&self, command: TransferCommand) -> Result<TransferAdmission, String> {
        command.validate()?;
        let job_id = self
            .manager
            .submit_job(
                command.user_id,
                command.name,
                command.transfer_type,
                command.source_connection_id,
                command.source_path,
                command.destination_connection_id,
                command.destination_path,
            )
            .await?;
        Ok(TransferAdmission { job_id })
    }

    pub async fn get_job(&self, job_id: &str) -> Option<TransferJob> {
        self.manager.get_job(job_id).await
    }

    pub async fn list_jobs(
        &self,
        user_id: Option<&str>,
        is_admin: bool,
        include_dismissed: bool,
    ) -> Vec<TransferJob> {
        self.manager
            .list_jobs(user_id, is_admin, include_dismissed)
            .await
    }

    pub async fn retry_job(
        &self,
        job_id: &str,
        user_id: Option<&str>,
        is_admin: bool,
    ) -> Result<bool, RetryTransferError> {
        self.manager.retry_job(job_id, user_id, is_admin).await
    }

    /// Compatibility escape hatch for code not migrated yet. New code should
    /// use explicit TransferEngine methods instead of reaching into the manager.
    pub fn legacy_manager(&self) -> &TransferManager {
        &self.manager
    }
}
