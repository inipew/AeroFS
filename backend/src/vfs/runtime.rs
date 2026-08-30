use crate::domain::{Capabilities, RetryPolicy};
use crate::errors::VfsError;
use crate::vfs::lifecycle::ProviderState;
use crate::vfs::FileSystem;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore, SemaphorePermit};

/// RAII Guard that holds a lease reference to an active storage connection.
pub struct ConnectionLeaseGuard {
    leases: Arc<AtomicUsize>,
    last_active: Arc<RwLock<Instant>>,
}

impl Drop for ConnectionLeaseGuard {
    fn drop(&mut self) {
        self.leases.fetch_sub(1, Ordering::SeqCst);
        let la = Arc::clone(&self.last_active);
        tokio::spawn(async move {
            let mut w = la.write().await;
            *w = Instant::now();
        });
    }
}

/// Centralized storage runtime combining provider execution, capabilities snapshot,
/// typed retry policy, lifecycle state machine, connection-wide concurrency budgeting,
/// and reference-counted lease tracking (Plan 65).
pub struct StorageRuntime {
    pub connection_id: String,
    pub provider: Arc<dyn FileSystem>,
    pub capabilities: Capabilities,
    pub retry: RetryPolicy,
    pub semaphore: Arc<Semaphore>,
    pub state: Arc<RwLock<ProviderState>>,
    pub active_leases: Arc<AtomicUsize>,
    pub last_active: Arc<RwLock<Instant>>,
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
        let permits = if max_concurrency == 0 {
            64
        } else {
            max_concurrency
        };
        let semaphore = Arc::new(Semaphore::new(permits));
        let state = Arc::new(RwLock::new(ProviderState::Ready));
        let active_leases = Arc::new(AtomicUsize::new(0));
        let last_active = Arc::new(RwLock::new(Instant::now()));

        Self {
            connection_id: conn_id,
            provider,
            capabilities,
            retry,
            semaphore,
            state,
            active_leases,
            last_active,
        }
    }

    pub async fn state(&self) -> ProviderState {
        self.state.read().await.clone()
    }

    pub async fn set_state(&self, new_state: ProviderState) {
        let mut s = self.state.write().await;
        *s = new_state;
    }

    pub async fn is_ready(&self) -> bool {
        self.state.read().await.is_ready()
    }

    pub async fn is_operational(&self) -> bool {
        self.state.read().await.is_operational()
    }

    pub fn provider(&self) -> &Arc<dyn FileSystem> {
        &self.provider
    }

    pub fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    /// Acquire an active connection lease (for Panels, Transfers, and Sync jobs)
    pub async fn acquire_lease(&self) -> ConnectionLeaseGuard {
        self.active_leases.fetch_add(1, Ordering::SeqCst);
        {
            let mut la = self.last_active.write().await;
            *la = Instant::now();
        }
        ConnectionLeaseGuard {
            leases: Arc::clone(&self.active_leases),
            last_active: Arc::clone(&self.last_active),
        }
    }

    /// Total count of active leases currently holding this connection
    pub fn active_leases_count(&self) -> usize {
        self.active_leases.load(Ordering::Relaxed)
    }

    /// Check if connection is idle with no active leases beyond specified TTL
    pub async fn is_idle(&self, ttl: Duration) -> bool {
        if self.active_leases_count() > 0 {
            return false;
        }
        let la = self.last_active.read().await;
        la.elapsed() >= ttl
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
