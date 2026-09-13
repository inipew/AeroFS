use crate::domain::{Capabilities, FileEntry, FileMetadata, VfsPath};
use crate::errors::VfsError;
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;
use std::time::Duration;
use tokio::io::AsyncRead;

pub type AsyncReadBox = Box<dyn AsyncRead + Send + Unpin>;
pub type FileStreamBox = Pin<Box<dyn Stream<Item = Result<FileEntry, VfsError>> + Send + 'static>>;

/// Optional extension trait for storage providers that support direct pre-signed URLs (e.g. S3 / S3-compatible)
#[async_trait]
pub trait PresignSupport: Send + Sync {
    /// Generate a pre-signed URL for direct browser-to-storage download
    async fn presign_read_url(&self, path: &VfsPath, expire: Duration) -> Result<String, VfsError>;

    async fn presign_write_url(
        &self,
        path: &VfsPath,
        expire: Duration,
    ) -> Result<String, VfsError>;
}

#[async_trait]
pub trait FileSystem: Send + Sync + 'static {
    /// Returns the provider operational capabilities
    fn capabilities(&self) -> Capabilities;

    /// Whether I/O for this provider consumes local-disk rather than network capacity.
    /// Remote/custom providers default to network admission unless they explicitly opt in.
    fn is_local(&self) -> bool {
        false
    }

    /// Return an asynchronous stream of directory entries (OpenDAL-native streaming primitive).
    ///
    /// Streams should stay cheap: provider-specific metadata that requires extra local syscalls or
    /// remote requests may be omitted and added later through `enrich_listing_entries` once callers
    /// have reduced the result set to the entries they actually need.
    async fn list_stream(&self, path: &VfsPath) -> Result<FileStreamBox, VfsError>;

    /// Enrich a bounded set of directory entries with provider-specific metadata.
    ///
    /// The default is a no-op. Local filesystem implementations use this hook to batch permission
    /// metadata work outside Tokio's async worker threads.
    async fn enrich_listing_entries(&self, entries: &mut [FileEntry]) -> Result<(), VfsError> {
        let _ = entries;
        Ok(())
    }

    /// List all entries in a directory (default implementation collects from list_stream)
    async fn list(&self, path: &VfsPath) -> Result<Vec<FileEntry>, VfsError> {
        use futures::StreamExt;
        let mut stream = self.list_stream(path).await?;
        let mut entries = Vec::new();
        while let Some(res) = stream.next().await {
            entries.push(res?);
        }
        self.enrich_listing_entries(&mut entries).await?;
        Ok(entries)
    }

    /// Retrieve detailed metadata for a file or directory
    async fn stat(&self, path: &VfsPath) -> Result<FileMetadata, VfsError>;

    /// Open a readable stream for a file
    async fn read_stream(&self, path: &VfsPath) -> Result<AsyncReadBox, VfsError>;

    /// Open a readable stream for a specific byte range (offset, length) without mandatory stat()
    async fn read_range(
        &self,
        path: &VfsPath,
        offset: u64,
        length: u64,
    ) -> Result<AsyncReadBox, VfsError>;

    /// Write from an input stream to a file (atomic if supported)
    async fn write_stream(&self, path: &VfsPath, input: AsyncReadBox) -> Result<(), VfsError>;

    /// Create an empty file
    async fn create_file(&self, path: &VfsPath) -> Result<(), VfsError>;

    /// Create a directory (and any necessary parent directories)
    async fn create_dir(&self, path: &VfsPath) -> Result<(), VfsError>;

    /// Delete a file or directory (recursive for directories)
    async fn delete(&self, path: &VfsPath) -> Result<(), VfsError>;

    /// Rename/move within the same provider
    async fn rename(&self, from: &VfsPath, to: &VfsPath) -> Result<(), VfsError>;

    /// Copy within the same provider
    async fn copy(&self, from: &VfsPath, to: &VfsPath) -> Result<(), VfsError>;

    /// Set permissions (octal string e.g. "0755", "0600") on a file or directory
    async fn set_permissions(&self, path: &VfsPath, permissions: &str) -> Result<(), VfsError> {
        let _ = path;
        let _ = permissions;
        Ok(())
    }

    /// Access optional PresignSupport capability if implemented by this provider
    fn as_presign(&self) -> Option<&dyn PresignSupport> {
        None
    }
}
