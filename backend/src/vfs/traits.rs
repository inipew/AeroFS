use crate::domain::{Capabilities, FileEntry, FileMetadata, VfsPath};
use crate::errors::VfsError;
use async_trait::async_trait;
use tokio::io::AsyncRead;

pub type AsyncReadBox = Box<dyn AsyncRead + Send + Unpin>;

#[async_trait]
pub trait FileSystem: Send + Sync + 'static {
    /// Returns the provider operational capabilities
    fn capabilities(&self) -> Capabilities;

    /// List entries in a directory
    async fn list(&self, path: &VfsPath) -> Result<Vec<FileEntry>, VfsError>;

    /// Retrieve detailed metadata for a file or directory
    async fn stat(&self, path: &VfsPath) -> Result<FileMetadata, VfsError>;

    /// Open a readable stream for a file
    async fn read_stream(&self, path: &VfsPath) -> Result<AsyncReadBox, VfsError>;

    /// Open a readable stream for a specific byte range (offset, length)
    async fn read_range(&self, path: &VfsPath, offset: u64, length: u64) -> Result<AsyncReadBox, VfsError>;

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
}
