use crate::domain::{FileKind, VfsPath};
use crate::sync::models::FileManifest;
use crate::vfs::FileSystem;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

const MAX_DEPTH: usize = 64;
const MAX_ENTRIES: usize = 100_000;

pub struct VfsScanner;

impl VfsScanner {
    /// Recursively scan a directory, returning a flat list of FileManifest entries.
    /// Paths are relative to `base_path`. Depth bounded to 64. Count bounded to 100_000.
    pub async fn scan_directory(
        fs: &Arc<dyn FileSystem>,
        conn_id: &str,
        base_path: &str,
        cancel: &CancellationToken,
    ) -> anyhow::Result<Vec<FileManifest>> {
        let mut results = Vec::new();
        let mut count = 0usize;
        Self::scan_recursive(
            fs,
            conn_id,
            base_path,
            "",
            0,
            cancel,
            &mut results,
            &mut count,
        )
        .await?;
        Ok(results)
    }

    #[allow(clippy::too_many_arguments)]
    async fn scan_recursive(
        fs: &Arc<dyn FileSystem>,
        conn_id: &str,
        base_path: &str,
        rel_path: &str,
        depth: usize,
        cancel: &CancellationToken,
        results: &mut Vec<FileManifest>,
        count: &mut usize,
    ) -> anyhow::Result<()> {
        if depth >= MAX_DEPTH || *count >= MAX_ENTRIES || cancel.is_cancelled() {
            return Ok(());
        }

        let full_path = if rel_path.is_empty() {
            base_path.to_string()
        } else {
            let sep = if base_path.ends_with('/') { "" } else { "/" };
            format!("{}{}{}", base_path, sep, rel_path)
        };

        let vfs_path = VfsPath::new(conn_id, full_path)?;
        let mut stream = fs.list_stream(&vfs_path).await?;
        
        use futures::StreamExt;
        let mut entries = Vec::new();
        while let Some(res) = stream.next().await {
            if cancel.is_cancelled() {
                return Ok(());
            }
            if *count >= MAX_ENTRIES {
                break;
            }
            let entry = res?;
            entries.push(entry);
        }

        for entry in entries {
            if cancel.is_cancelled() {
                break;
            }

            let entry_rel = if rel_path.is_empty() {
                entry.name.clone()
            } else {
                format!("{}/{}", rel_path, entry.name)
            };

            let entry_vfs = VfsPath::new(conn_id, &entry.path)?;
            let meta = fs.stat(&entry_vfs).await?;

            results.push(FileManifest {
                path: entry_rel.clone(),
                kind: meta.kind,
                size: meta.size,
                modified_at: meta.modified_at,
                content_hash: None,
                etag: Some(meta.etag),
            });
            *count += 1;

            if meta.kind == FileKind::Directory {
                Box::pin(Self::scan_recursive(
                    fs,
                    conn_id,
                    base_path,
                    &entry_rel,
                    depth + 1,
                    cancel,
                    results,
                    count,
                ))
                .await?;
            }
        }

        Ok(())
    }
}
