use crate::domain::{Capabilities, RetryPolicy};
use crate::errors::VfsError;
use crate::vfs::FileSystem;
use std::sync::Arc;
use tokio::sync::{Semaphore, SemaphorePermit};

/// Centralized storage runtime combining provider execution, capabilities snapshot,
/// typed retry policy, and connection-wide concurrency budgeting.
pub struct StorageRuntime {
    pub connection_id: String,
    pub provider: Arc<dyn FileSystem>,
    pub capabilities: Capabilities,
    pub retry: RetryPolicy,
    pub semaphore: Arc<Semaphore>,
}

impl StorageRuntime {
    pub fn new(
        connection_id: impl Into<String>,
        provider: Arc<dyn FileSystem>,
        max_concurrency: usize,
    ) -> Self {
        let conn_id = connection_id.into();
        let capabilities = provider.capabilities().clone();
        let retry = RetryPolicy::default();
        let permits = if max_concurrency == 0 { 64 } else { max_concurrency };
        let semaphore = Arc::new(Semaphore::new(permits));

        Self {
            connection_id: conn_id,
            provider,
            capabilities,
            retry,
            semaphore,
        }
    }

    pub fn provider(&self) -> &Arc<dyn FileSystem> {
        &self.provider
    }

    pub fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    pub async fn acquire_permit(&self) -> Result<SemaphorePermit<'_>, VfsError> {
        self.semaphore.acquire().await.map_err(|e| {
            VfsError::IoError(format!(
                "Failed to acquire storage concurrency permit for connection '{}': {}",
                self.connection_id, e
            ))
        })
    }
}
