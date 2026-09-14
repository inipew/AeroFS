use crate::domain::{Capabilities, RetryPolicy, VfsPath};
use crate::errors::VfsError;
use crate::vfs::lifecycle::ProviderState;
use crate::vfs::{AsyncReadBox, FileSystem};
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, ReadBuf};
use tokio::sync::{OwnedSemaphorePermit, RwLock, Semaphore, SemaphorePermit};

pub type ProviderLoader = Arc<dyn Fn() -> anyhow::Result<Arc<dyn FileSystem>> + Send + Sync>;

fn monotonic_millis() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START
        .get_or_init(Instant::now)
        .elapsed()
        .as_millis()
        .min(u64::MAX as u128) as u64
}

/// RAII guard for one active storage-connection lease.
///
/// `last_active` is updated before the final lease is released so an idle reaper can never
/// observe `active_leases == 0` together with a stale timestamp from before the operation.
pub struct ConnectionLeaseGuard {
    leases: Arc<AtomicUsize>,
    last_active_ms: Arc<AtomicU64>,
}

impl Drop for ConnectionLeaseGuard {
    fn drop(&mut self) {
        self.last_active_ms
            .store(monotonic_millis(), Ordering::Release);
        self.leases.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Centralized storage runtime combining provider execution, capabilities snapshot,
/// typed retry policy, lifecycle state machine, connection-wide concurrency budgeting,
/// and reference-counted lease tracking.
///
/// Remote providers can be reclaimable. The registry keeps this lightweight runtime and its
/// `BudgetedFileSystem` proxy alive while the heavyweight provider/operator may be dropped after
/// an idle TTL. The provider is rebuilt lazily on the next operation through `provider_loader`.
pub struct StorageRuntime {
    pub connection_id: String,
    provider: Arc<RwLock<Option<Arc<dyn FileSystem>>>>,
    provider_loader: Option<ProviderLoader>,
    supports_presign: bool,
    pub capabilities: Capabilities,
    pub retry: RetryPolicy,
    pub semaphore: Arc<Semaphore>,
    pub state: Arc<RwLock<ProviderState>>,
    pub active_leases: Arc<AtomicUsize>,
    last_active_ms: Arc<AtomicU64>,
}

impl StorageRuntime {
    pub fn new(
        connection_id: impl Into<String>,
        provider: Arc<dyn FileSystem>,
        max_concurrency: usize,
    ) -> Self {
        Self::new_inner(connection_id, provider, max_concurrency, None)
    }

    pub fn new_reclaimable(
        connection_id: impl Into<String>,
        provider: Arc<dyn FileSystem>,
        max_concurrency: usize,
        provider_loader: ProviderLoader,
    ) -> Self {
        Self::new_inner(
            connection_id,
            provider,
            max_concurrency,
            Some(provider_loader),
        )
    }

    fn new_inner(
        connection_id: impl Into<String>,
        provider: Arc<dyn FileSystem>,
        max_concurrency: usize,
        provider_loader: Option<ProviderLoader>,
    ) -> Self {
        let conn_id = connection_id.into();
        let capabilities = provider.capabilities().clone();
        let supports_presign = provider.as_presign().is_some();
        let retry = RetryPolicy::default();
        let permits = if max_concurrency == 0 {
            64
        } else {
            max_concurrency
        };

        Self {
            connection_id: conn_id,
            provider: Arc::new(RwLock::new(Some(provider))),
            provider_loader,
            supports_presign,
            capabilities,
            retry,
            semaphore: Arc::new(Semaphore::new(permits)),
            state: Arc::new(RwLock::new(ProviderState::Ready)),
            active_leases: Arc::new(AtomicUsize::new(0)),
            last_active_ms: Arc::new(AtomicU64::new(monotonic_millis())),
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

    pub fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    pub fn is_reclaimable(&self) -> bool {
        self.provider_loader.is_some()
    }

    /// Resolve the heavyweight provider, lazily rebuilding it if an idle reaper reclaimed it.
    pub async fn provider_for_operation(&self) -> Result<Arc<dyn FileSystem>, VfsError> {
        if let Some(provider) = self.provider.read().await.as_ref().cloned() {
            return Ok(provider);
        }

        let mut provider_slot = self.provider.write().await;
        if let Some(provider) = provider_slot.as_ref().cloned() {
            return Ok(provider);
        }

        let loader = self.provider_loader.as_ref().ok_or_else(|| {
            VfsError::ConnectionError(format!(
                "Provider '{}' is not available and has no reload factory",
                self.connection_id
            ))
        })?;
        let provider = loader().map_err(|error| {
            VfsError::ConnectionError(format!(
                "Failed to restore provider '{}': {}",
                self.connection_id, error
            ))
        })?;
        *provider_slot = Some(provider.clone());
        self.last_active_ms
            .store(monotonic_millis(), Ordering::Release);
        tracing::debug!(connection_id = %self.connection_id, "Restored reclaimed provider runtime");
        Ok(provider)
    }

    /// Acquire an active connection lease (panels, transfers, sync, and direct VFS operations).
    pub async fn acquire_lease(&self) -> ConnectionLeaseGuard {
        self.active_leases.fetch_add(1, Ordering::AcqRel);
        self.last_active_ms
            .store(monotonic_millis(), Ordering::Release);
        ConnectionLeaseGuard {
            leases: Arc::clone(&self.active_leases),
            last_active_ms: Arc::clone(&self.last_active_ms),
        }
    }

    pub fn active_leases_count(&self) -> usize {
        self.active_leases.load(Ordering::Acquire)
    }

    pub fn is_idle(&self, ttl: Duration) -> bool {
        if self.active_leases_count() > 0 {
            return false;
        }
        let now = monotonic_millis();
        let last = self.last_active_ms.load(Ordering::Acquire);
        now.saturating_sub(last) >= ttl.as_millis().min(u64::MAX as u128) as u64
    }

    /// Drop the heavyweight remote provider if it is truly idle.
    ///
    /// The lease count is checked both before and after taking the provider write lock, closing
    /// the race where a new operation starts while the reaper is deciding to reclaim it.
    pub async fn reclaim_if_idle(&self, ttl: Duration) -> bool {
        if !self.is_reclaimable() || !self.is_idle(ttl) {
            return false;
        }

        let mut provider_slot = self.provider.write().await;
        if !self.is_idle(ttl) {
            return false;
        }
        if provider_slot.take().is_some() {
            tracing::debug!(connection_id = %self.connection_id, "Reclaimed idle provider runtime");
            return true;
        }
        false
    }

    pub async fn acquire_permit(&self) -> Result<SemaphorePermit<'_>, VfsError> {
        self.semaphore.acquire().await.map_err(|e| {
            VfsError::IoError(format!(
                "Failed to acquire storage concurrency permit for connection '{}': {}",
                self.connection_id, e
            ))
        })
    }

    pub async fn acquire_owned_permit(&self) -> Result<OwnedSemaphorePermit, VfsError> {
        self.semaphore.clone().acquire_owned().await.map_err(|e| {
            VfsError::IoError(format!(
                "Failed to acquire storage concurrency permit for connection '{}': {}",
                self.connection_id, e
            ))
        })
    }
}

/// File-system decorator that makes `StorageRuntime`'s concurrency budget and lease tracking
/// invariants rather than optional conventions.
#[derive(Clone)]
pub struct BudgetedFileSystem {
    runtime: Arc<StorageRuntime>,
}

impl BudgetedFileSystem {
    pub fn new(runtime: Arc<StorageRuntime>) -> Self {
        Self { runtime }
    }

    pub fn runtime(&self) -> &Arc<StorageRuntime> {
        &self.runtime
    }
}

struct PermitRead {
    inner: AsyncReadBox,
    _permit: OwnedSemaphorePermit,
    _lease: ConnectionLeaseGuard,
}

impl AsyncRead for PermitRead {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut *self.inner).poll_read(cx, buf)
    }
}

struct PermitStream {
    inner: crate::vfs::traits::FileStreamBox,
    _permit: OwnedSemaphorePermit,
    _lease: ConnectionLeaseGuard,
}

impl Stream for PermitStream {
    type Item = Result<crate::domain::FileEntry, VfsError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.as_mut().poll_next(cx)
    }
}

#[async_trait]
impl crate::vfs::traits::PresignSupport for BudgetedFileSystem {
    async fn presign_read_url(
        &self,
        path: &VfsPath,
        expire: Duration,
    ) -> Result<String, VfsError> {
        let _lease = self.runtime.acquire_lease().await;
        let _permit = self.runtime.acquire_owned_permit().await?;
        let provider = self.runtime.provider_for_operation().await?;
        let presign = provider.as_presign().ok_or_else(|| {
            VfsError::NotSupported("Provider does not support presigned downloads".into())
        })?;
        presign.presign_read_url(path, expire).await
    }

    async fn presign_write_url(
        &self,
        path: &VfsPath,
        expire: Duration,
    ) -> Result<String, VfsError> {
        let _lease = self.runtime.acquire_lease().await;
        let _permit = self.runtime.acquire_owned_permit().await?;
        let provider = self.runtime.provider_for_operation().await?;
        let presign = provider.as_presign().ok_or_else(|| {
            VfsError::NotSupported("Provider does not support presigned uploads".into())
        })?;
        presign.presign_write_url(path, expire).await
    }
}

#[async_trait]
impl FileSystem for BudgetedFileSystem {
    fn capabilities(&self) -> Capabilities {
        self.runtime.capabilities.clone()
    }

    async fn list_stream(
        &self,
        path: &VfsPath,
    ) -> Result<crate::vfs::traits::FileStreamBox, VfsError> {
        let lease = self.runtime.acquire_lease().await;
        let permit = self.runtime.acquire_owned_permit().await?;
        let provider = self.runtime.provider_for_operation().await?;
        let inner = provider.list_stream(path).await?;
        Ok(Box::pin(PermitStream {
            inner,
            _permit: permit,
            _lease: lease,
        }))
    }

    async fn stat(&self, path: &VfsPath) -> Result<crate::domain::FileMetadata, VfsError> {
        let _lease = self.runtime.acquire_lease().await;
        let _permit = self.runtime.acquire_owned_permit().await?;
        self.runtime.provider_for_operation().await?.stat(path).await
    }

    async fn read_stream(&self, path: &VfsPath) -> Result<AsyncReadBox, VfsError> {
        let lease = self.runtime.acquire_lease().await;
        let permit = self.runtime.acquire_owned_permit().await?;
        let provider = self.runtime.provider_for_operation().await?;
        let inner = provider.read_stream(path).await?;
        Ok(Box::new(PermitRead {
            inner,
            _permit: permit,
            _lease: lease,
        }))
    }

    async fn read_range(
        &self,
        path: &VfsPath,
        offset: u64,
        length: u64,
    ) -> Result<AsyncReadBox, VfsError> {
        let lease = self.runtime.acquire_lease().await;
        let permit = self.runtime.acquire_owned_permit().await?;
        let provider = self.runtime.provider_for_operation().await?;
        let inner = provider.read_range(path, offset, length).await?;
        Ok(Box::new(PermitRead {
            inner,
            _permit: permit,
            _lease: lease,
        }))
    }

    async fn write_stream(&self, path: &VfsPath, input: AsyncReadBox) -> Result<(), VfsError> {
        let _lease = self.runtime.acquire_lease().await;
        let _permit = self.runtime.acquire_owned_permit().await?;
        self.runtime
            .provider_for_operation()
            .await?
            .write_stream(path, input)
            .await
    }

    async fn create_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        let _lease = self.runtime.acquire_lease().await;
        let _permit = self.runtime.acquire_owned_permit().await?;
        self.runtime.provider_for_operation().await?.create_file(path).await
    }

    async fn create_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        let _lease = self.runtime.acquire_lease().await;
        let _permit = self.runtime.acquire_owned_permit().await?;
        self.runtime.provider_for_operation().await?.create_dir(path).await
    }

    async fn delete(&self, path: &VfsPath) -> Result<(), VfsError> {
        let _lease = self.runtime.acquire_lease().await;
        let _permit = self.runtime.acquire_owned_permit().await?;
        self.runtime.provider_for_operation().await?.delete(path).await
    }

    async fn rename(&self, from: &VfsPath, to: &VfsPath) -> Result<(), VfsError> {
        let _lease = self.runtime.acquire_lease().await;
        let _permit = self.runtime.acquire_owned_permit().await?;
        self.runtime.provider_for_operation().await?.rename(from, to).await
    }

    async fn copy(&self, from: &VfsPath, to: &VfsPath) -> Result<(), VfsError> {
        let _lease = self.runtime.acquire_lease().await;
        let _permit = self.runtime.acquire_owned_permit().await?;
        self.runtime.provider_for_operation().await?.copy(from, to).await
    }

    async fn set_permissions(&self, path: &VfsPath, permissions: &str) -> Result<(), VfsError> {
        let _lease = self.runtime.acquire_lease().await;
        let _permit = self.runtime.acquire_owned_permit().await?;
        self.runtime
            .provider_for_operation()
            .await?
            .set_permissions(path, permissions)
            .await
    }

    fn as_presign(&self) -> Option<&dyn crate::vfs::traits::PresignSupport> {
        if self.runtime.supports_presign {
            Some(self as _)
        } else {
            None
        }
    }
}
