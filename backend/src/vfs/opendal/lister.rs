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

/// OpenDAL-native streaming directory lister
pub struct OpenDalLister {
    lister: opendal::Lister,
    base_vfs_path: VfsPath,
    local_root: Option<PathBuf>,
}

impl OpenDalLister {
    pub fn new(
        lister: opendal::Lister,
        base_vfs_path: VfsPath,
        local_root: Option<PathBuf>,
    ) -> Self {
        Self {
            lister,
            base_vfs_path,
            local_root,
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

            if let Some(mut mapped) = map_opendal_entry(&entry, &self.base_vfs_path) {
                #[cfg(unix)]
                if let Some(ref root) = self.local_root {
                    use std::os::unix::fs::PermissionsExt;
                    let abs_child = root.join(mapped.path.trim_start_matches('/'));
                    if let Ok(sym_meta) = std::fs::symlink_metadata(&abs_child) {
                        let mode = sym_meta.permissions().mode() & 0o7777;
                        mapped.permissions = Some(format!("{:04o}", mode));
                    }
                }
                return Ok(Some(mapped));
            }
        }
        Ok(None)
    }
}

/// Create a BoxStream from OpenDAL operator and target path using zero-overhead stream unfolding
pub async fn create_opendal_stream(
    operator: &Operator,
    list_target: &str,
    base_vfs_path: &VfsPath,
    local_root: Option<PathBuf>,
) -> Result<FileStreamBox, VfsError> {
    let lister = operator.lister(list_target).await.map_err(|e| {
        map_opendal_error(
            e,
            &format!("Failed to init lister for '{}'", base_vfs_path.path),
        )
    })?;

    let vfs_path_clone = base_vfs_path.clone();
    let stream = futures::stream::unfold(
        (lister, vfs_path_clone, local_root),
        |(mut lister, base_path, root)| async move {
            use futures::StreamExt;
            while let Some(res) = lister.next().await {
                match res {
                    Ok(entry) => {
                        if let Some(mut mapped) = map_opendal_entry(&entry, &base_path) {
                            #[cfg(unix)]
                            if let Some(ref root_dir) = root {
                                use std::os::unix::fs::PermissionsExt;
                                let abs_child = root_dir.join(mapped.path.trim_start_matches('/'));
                                if let Ok(sym_meta) = std::fs::symlink_metadata(&abs_child) {
                                    let mode = sym_meta.permissions().mode() & 0o7777;
                                    mapped.permissions = Some(format!("{:04o}", mode));
                                }
                            }
                            return Some((Ok(mapped), (lister, base_path, root)));
                        }
                    }
                    Err(e) => {
                        let err =
                            map_opendal_error(e, &format!("Lister error for '{}'", base_path.path));
                        return Some((Err(err), (lister, base_path, root)));
                    }
                }
            }
            None
        },
    );

    Ok(Box::pin(stream))
}
