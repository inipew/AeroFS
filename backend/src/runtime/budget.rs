use std::sync::Arc;
use tokio::sync::{AcquireError, OwnedSemaphorePermit, Semaphore};

/// A logical unit of resource pressure. Every acquisition follows the same
/// semaphore order: global -> local -> network -> transfer -> archive -> search.
/// This keeps compound permits deadlock-free while making the system-wide budget
/// authoritative across subsystems.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceClass {
    LocalIo,
    NetworkIo,
    MixedIo,
    TransferLocal,
    TransferNetwork,
    TransferMixed,
    ArchiveLocal,
    ArchiveNetwork,
    SearchLocal,
    SearchNetwork,
}

/// Owns all permits for one admitted operation. Dropping this value releases the
/// complete resource unit atomically from the caller's perspective.
pub struct ResourcePermit {
    _global: OwnedSemaphorePermit,
    _local: Option<OwnedSemaphorePermit>,
    _network: Option<OwnedSemaphorePermit>,
    _transfer: Option<OwnedSemaphorePermit>,
    _archive: Option<OwnedSemaphorePermit>,
    _search: Option<OwnedSemaphorePermit>,
}

/// System-wide coordinated concurrency budget to prevent multiplicative overload.
#[derive(Clone)]
pub struct ResourceBudget {
    global_io: Arc<Semaphore>,
    local_disk: Arc<Semaphore>,
    network: Arc<Semaphore>,
    transfer: Arc<Semaphore>,
    archive: Arc<Semaphore>,
    search: Arc<Semaphore>,
}

impl Default for ResourceBudget {
    fn default() -> Self {
        Self::with_limits(32, 16, 16, 4, 4, 4)
    }
}

impl ResourceBudget {
    /// Compatibility constructor. Transfer capacity defaults to four workers.
    pub fn new(
        global_io_permits: usize,
        local_disk_permits: usize,
        network_permits: usize,
        archive_permits: usize,
        search_permits: usize,
    ) -> Self {
        Self::with_limits(
            global_io_permits,
            local_disk_permits,
            network_permits,
            4,
            archive_permits,
            search_permits,
        )
    }

    pub fn with_limits(
        global_io_permits: usize,
        local_disk_permits: usize,
        network_permits: usize,
        transfer_permits: usize,
        archive_permits: usize,
        search_permits: usize,
    ) -> Self {
        Self {
            global_io: Arc::new(Semaphore::new(global_io_permits.max(1))),
            local_disk: Arc::new(Semaphore::new(local_disk_permits.max(1))),
            network: Arc::new(Semaphore::new(network_permits.max(1))),
            transfer: Arc::new(Semaphore::new(transfer_permits.max(1))),
            archive: Arc::new(Semaphore::new(archive_permits.max(1))),
            search: Arc::new(Semaphore::new(search_permits.max(1))),
        }
    }

    pub async fn acquire(&self, class: ResourceClass) -> Result<ResourcePermit, AcquireError> {
        let global = self.global_io.clone().acquire_owned().await?;

        let needs_local = matches!(
            class,
            ResourceClass::LocalIo
                | ResourceClass::MixedIo
                | ResourceClass::TransferLocal
                | ResourceClass::TransferMixed
                | ResourceClass::ArchiveLocal
                | ResourceClass::SearchLocal
        );
        let local = if needs_local {
            Some(self.local_disk.clone().acquire_owned().await?)
        } else {
            None
        };

        let needs_network = matches!(
            class,
            ResourceClass::NetworkIo
                | ResourceClass::MixedIo
                | ResourceClass::TransferNetwork
                | ResourceClass::TransferMixed
                | ResourceClass::ArchiveNetwork
                | ResourceClass::SearchNetwork
        );
        let network = if needs_network {
            Some(self.network.clone().acquire_owned().await?)
        } else {
            None
        };

        let transfer = if matches!(
            class,
            ResourceClass::TransferLocal
                | ResourceClass::TransferNetwork
                | ResourceClass::TransferMixed
        ) {
            Some(self.transfer.clone().acquire_owned().await?)
        } else {
            None
        };

        let archive = if matches!(
            class,
            ResourceClass::ArchiveLocal | ResourceClass::ArchiveNetwork
        ) {
            Some(self.archive.clone().acquire_owned().await?)
        } else {
            None
        };

        let search = if matches!(
            class,
            ResourceClass::SearchLocal | ResourceClass::SearchNetwork
        ) {
            Some(self.search.clone().acquire_owned().await?)
        } else {
            None
        };

        Ok(ResourcePermit {
            _global: global,
            _local: local,
            _network: network,
            _transfer: transfer,
            _archive: archive,
            _search: search,
        })
    }

    /// Scheduler-facing transfer slot. TransferManager uses this exact semaphore rather
    /// than constructing an independent one, so runtime settings and admission control
    /// operate on the same capacity source.
    pub fn transfer_semaphore(&self) -> Arc<Semaphore> {
        self.transfer.clone()
    }

    pub fn available_archive(&self) -> usize {
        self.archive.available_permits()
    }

    pub fn available_search(&self) -> usize {
        self.search.available_permits()
    }

    pub fn available_transfer(&self) -> usize {
        self.transfer.available_permits()
    }

    /// Preserve the existing live-settings behavior: raising the configured transfer
    /// limit takes effect immediately. Lowering remains a soft limit until currently
    /// available/active permits naturally turn over, matching the prior manager behavior.
    pub fn add_transfer_permits(&self, additional: usize) {
        if additional > 0 {
            self.transfer.add_permits(additional);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn compound_permit_consumes_all_required_classes() {
        let budget = ResourceBudget::with_limits(2, 1, 1, 1, 1, 1);
        let permit = budget.acquire(ResourceClass::TransferMixed).await.unwrap();
        assert_eq!(budget.available_transfer(), 0);
        drop(permit);
        assert_eq!(budget.available_transfer(), 1);
    }

    #[tokio::test]
    async fn search_capacity_is_shared_globally() {
        let budget = ResourceBudget::with_limits(4, 4, 4, 2, 2, 1);
        let permit = budget.acquire(ResourceClass::SearchNetwork).await.unwrap();
        assert_eq!(budget.available_search(), 0);
        drop(permit);
        assert_eq!(budget.available_search(), 1);
    }
}
