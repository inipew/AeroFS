use std::sync::Arc;
use tokio::sync::{Semaphore, SemaphorePermit};

/// System-wide coordinated concurrency budget to prevent multiplicative overload.
#[derive(Clone)]
pub struct ResourceBudget {
    pub global_io: Arc<Semaphore>,
    pub local_disk: Arc<Semaphore>,
    pub network: Arc<Semaphore>,
    pub archive: Arc<Semaphore>,
    pub search: Arc<Semaphore>,
}

impl Default for ResourceBudget {
    fn default() -> Self {
        Self {
            global_io: Arc::new(Semaphore::new(32)),
            local_disk: Arc::new(Semaphore::new(16)),
            network: Arc::new(Semaphore::new(16)),
            archive: Arc::new(Semaphore::new(4)),
            search: Arc::new(Semaphore::new(4)),
        }
    }
}

impl ResourceBudget {
    pub fn new(
        global_io_permits: usize,
        local_disk_permits: usize,
        network_permits: usize,
        archive_permits: usize,
        search_permits: usize,
    ) -> Self {
        Self {
            global_io: Arc::new(Semaphore::new(global_io_permits)),
            local_disk: Arc::new(Semaphore::new(local_disk_permits)),
            network: Arc::new(Semaphore::new(network_permits)),
            archive: Arc::new(Semaphore::new(archive_permits)),
            search: Arc::new(Semaphore::new(search_permits)),
        }
    }

    pub async fn acquire_global_io(&self) -> Option<SemaphorePermit<'_>> {
        self.global_io.acquire().await.ok()
    }

    pub async fn acquire_local_disk(&self) -> Option<SemaphorePermit<'_>> {
        self.local_disk.acquire().await.ok()
    }

    pub async fn acquire_network(&self) -> Option<SemaphorePermit<'_>> {
        self.network.acquire().await.ok()
    }

    pub async fn acquire_archive(&self) -> Option<SemaphorePermit<'_>> {
        self.archive.acquire().await.ok()
    }

    pub async fn acquire_search(&self) -> Option<SemaphorePermit<'_>> {
        self.search.acquire().await.ok()
    }
}
