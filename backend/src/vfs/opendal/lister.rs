use super::error::map_opendal_error;
use super::metadata::map_opendal_entry;
use crate::domain::{FileEntry, VfsPath};
use crate::errors::VfsError;
use async_trait::async_trait;
use futures::Stream;
use opendal::Operator;
use std::path::PathBuf;
use std::pin::Pin;

pub type FileStreamBox = Pin<Box<dyn Stream<Item = Result<FileEntry, VfsError>> + Send + 'static>>;

/// Trait representing an active directory stream iterator
#[async_trait]
pub trait FileLister: Send + Sync {
    async fn next_entry(&mut self) -> Result<Option<FileEntry>, VfsError>;
}

/// OpenDAL-native streaming directory lister.
///
/// Expensive provider-specific metadata (for example POSIX permissions on local
/// filesystems) is intentionally not enriched here. Directory streams can visit
/// very large directories even when callers only need one bounded page, so doing
/// synchronous metadata syscalls per traversed entry would block an async worker
/// and turn pagination into O(N) local syscalls. Callers that need rich listing
/// metadata should use `FileSystem::enrich_listing_entries` after pagination.
pub struct OpenDalLister {
    lister: opendal::Lister,
    base_vfs_path: VfsPath,
}

impl OpenDalLister {
    pub fn new(
        lister: opendal::Lister,
        base_vfs_path: VfsPath,
        _local_root: Option<PathBuf>,
    ) -> Self {
        Self {
            lister,
            base_vfs_path,
        }
    }
}

#[async_trait]
impl FileLister for OpenDalLister {
    async fn next_entry(&mut self) -> Result<Option<FileEntry>, VfsError> {
        use futures::StreamExt;
        while let Some(res) = self.lister.next().await {
            let entry = res.map_err(|e| {
                map_opendal_error(
                    e,
                    &format!("Failed during listing of '{}'", self.base_vfs_path.path),
                )
            })?;

            if let Some(mapped) = map_opendal_entry(&entry, &self.base_vfs_path) {
                return Ok(Some(mapped));
            }
        }
        Ok(None)
    }
}

/// Create a BoxStream from OpenDAL operator and target path using zero-overhead stream unfolding.
///
/// `local_root` remains part of the signature for compatibility with existing
/// callers, but local permission enrichment is deliberately deferred until the
/// final bounded result page is known.
pub async fn create_opendal_stream(
    operator: &Operator,
    list_target: &str,
    base_vfs_path: &VfsPath,
    _local_root: Option<PathBuf>,
) -> Result<FileStreamBox, VfsError> {
    let lister = operator.lister(list_target).await.map_err(|e| {
        map_opendal_error(
            e,
            &format!("Failed to init lister for '{}'", base_vfs_path.path),
        )
    })?;

    let vfs_path_clone = base_vfs_path.clone();
    let stream = futures::stream::unfold(
        (lister, vfs_path_clone),
        |(mut lister, base_path)| async move {
            use futures::StreamExt;
            while let Some(res) = lister.next().await {
                match res {
                    Ok(entry) => {
                        if let Some(mapped) = map_opendal_entry(&entry, &base_path) {
                            return Some((Ok(mapped), (lister, base_path)));
                        }
                    }
                    Err(e) => {
                        let err =
                            map_opendal_error(e, &format!("Lister error for '{}'", base_path.path));
                        return Some((Err(err), (lister, base_path)));
                    }
                }
            }
            None
        },
    );

    Ok(Box::pin(stream))
}
