use crate::domain::{Capabilities, RetryPolicy, VfsPath};
use crate::errors::VfsError;
use crate::vfs::lifecycle::ProviderState;
use crate::vfs::{AsyncReadBox, FileSystem};
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, ReadBuf};
use tokio::sync::{OwnedSemaphorePermit, RwLock, Semaphore, SemaphorePermit};

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

    /// Returns the raw provider owned by this runtime. Application-facing
    /// callers should resolve providers through `ProviderRegistry`, which wraps
    /// this provider in `BudgetedFileSystem`.
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

    pub async fn acquire_owned_permit(&self) -> Result<OwnedSemaphorePermit, VfsError> {
        self.semaphore.clone().acquire_owned().await.map_err(|e| {
            VfsError::IoError(format!(
                "Failed to acquire storage concurrency permit for connection '{}': {}",
                self.connection_id, e
            ))
        })
    }
}

/// File-system decorator that makes `StorageRuntime`'s concurrency budget an
/// invariant rather than an optional convention. Every provider operation
/// acquires one connection-wide permit before touching the raw provider.
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
        let _permit = self.runtime.acquire_owned_permit().await?;
        let presign = self.runtime.provider.as_presign().ok_or_else(|| {
            VfsError::NotSupported("Provider does not support presigned downloads".into())
        })?;
        presign.presign_read_url(path, expire).await
    }

    async fn presign_write_url(
        &self,
        path: &VfsPath,
        expire: Duration,
    ) -> Result<String, VfsError> {
        let _permit = self.runtime.acquire_owned_permit().await?;
        let presign = self.runtime.provider.as_presign().ok_or_else(|| {
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
        let permit = self.runtime.acquire_owned_permit().await?;
        let inner = self.runtime.provider.list_stream(path).await?;
        Ok(Box::pin(PermitStream {
            inner,
            _permit: permit,
        }))
    }

    async fn stat(&self, path: &VfsPath) -> Result<crate::domain::FileMetadata, VfsError> {
        let _permit = self.runtime.acquire_owned_permit().await?;
        self.runtime.provider.stat(path).await
    }

    async fn read_stream(&self, path: &VfsPath) -> Result<AsyncReadBox, VfsError> {
        let permit = self.runtime.acquire_owned_permit().await?;
        let inner = self.runtime.provider.read_stream(path).await?;
        Ok(Box::new(PermitRead {
            inner,
            _permit: permit,
        }))
    }

    async fn read_range(
        &self,
        path: &VfsPath,
        offset: u64,
        length: u64,
    ) -> Result<AsyncReadBox, VfsError> {
        let permit = self.runtime.acquire_owned_permit().await?;
        let inner = self.runtime.provider.read_range(path, offset, length).await?;
        Ok(Box::new(PermitRead {
            inner,
            _permit: permit,
        }))
    }

    async fn write_stream(&self, path: &VfsPath, input: AsyncReadBox) -> Result<(), VfsError> {
        let _permit = self.runtime.acquire_owned_permit().await?;
        self.runtime.provider.write_stream(path, input).await
    }

    async fn create_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        let _permit = self.runtime.acquire_owned_permit().await?;
        self.runtime.provider.create_file(path).await
    }

    async fn create_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        let _permit = self.runtime.acquire_owned_permit().await?;
        self.runtime.provider.create_dir(path).await
    }

    async fn delete(&self, path: &VfsPath) -> Result<(), VfsError> {
        let _permit = self.runtime.acquire_owned_permit().await?;
        self.runtime.provider.delete(path).await
    }

    async fn rename(&self, from: &VfsPath, to: &VfsPath) -> Result<(), VfsError> {
        let _permit = self.runtime.acquire_owned_permit().await?;
        self.runtime.provider.rename(from, to).await
    }

    async fn copy(&self, from: &VfsPath, to: &VfsPath) -> Result<(), VfsError> {
        let _permit = self.runtime.acquire_owned_permit().await?;
        self.runtime.provider.copy(from, to).await
    }

    async fn set_permissions(&self, path: &VfsPath, permissions: &str) -> Result<(), VfsError> {
        let _permit = self.runtime.acquire_owned_permit().await?;
        self.runtime
            .provider
            .set_permissions(path, permissions)
            .await
    }

    fn as_presign(&self) -> Option<&dyn crate::vfs::traits::PresignSupport> {
        self.runtime.provider.as_presign().map(|_| self as _)
    }
}
