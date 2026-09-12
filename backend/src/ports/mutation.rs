use crate::domain::ConnectionId;
use crate::errors::AppError;
use async_trait::async_trait;

/// Opaque RAII lease held for the lifetime of a file mutation critical section.
/// Dropping the lease must release the corresponding destination reservation.
pub trait MutationLease: Send {}

impl<T: Send> MutationLease for T {}

/// Application-owned boundary for serializing mutations of one destination path.
/// Implementations decide whether contention waits or fails fast, but a successful
/// lease must remain valid until the returned guard is dropped.
#[async_trait]
pub trait MutationCoordinator: Send + Sync {
    async fn try_acquire(
        &self,
        connection: &ConnectionId,
        path: &str,
    ) -> Result<Box<dyn MutationLease>, AppError>;
}
