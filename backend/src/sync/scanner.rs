use crate::domain::{FileKind, VfsPath};
use crate::sync::models::FileManifest;
use crate::vfs::FileSystem;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

const MAX_DEPTH: usize = 64;
const MAX_ENTRIES: usize = 100_000;
pub const SCAN_BATCH_SIZE: usize = 256;
const SCAN_CHANNEL_DEPTH: usize = 2;

pub type ManifestBatch = anyhow::Result<Vec<FileManifest>>;

pub struct VfsScanner;

impl VfsScanner {
    /// Recursively scans a directory and emits bounded manifest batches.
    ///
    /// The producer applies backpressure through a small bounded channel. Neither a directory's
    /// full listing nor the full tree is materialized in memory. Traversal is depth-first so the
    /// recursion stack is bounded by `MAX_DEPTH`.
    pub fn scan_directory_batches(
        fs: Arc<dyn FileSystem>,
        conn_id: String,
        base_path: String,
        cancel: CancellationToken,
    ) -> mpsc::Receiver<ManifestBatch> {
        let (tx, rx) = mpsc::channel(SCAN_CHANNEL_DEPTH);

        tokio::spawn(async move {
            let mut batch = Vec::with_capacity(SCAN_BATCH_SIZE);
            let mut count = 0usize;
            let result = Self::scan_recursive_batched(
                &fs,
                &conn_id,
                &base_path,
                "",
                0,
                &cancel,
                &tx,
                &mut batch,
                &mut count,
            )
            .await;

            match result {
                Ok(()) => {
                    if !batch.is_empty() && !cancel.is_cancelled() {
                        let _ = tx.send(Ok(batch)).await;
                    }
                }
                Err(error) => {
                    let _ = tx.send(Err(error)).await;
                }
            }
        });

        rx
    }

    /// Compatibility helper for callers that explicitly need a materialized manifest.
    /// New sync pipeline code should use `scan_directory_batches`.
    pub async fn scan_directory(
        fs: &Arc<dyn FileSystem>,
        conn_id: &str,
        base_path: &str,
        cancel: &CancellationToken,
    ) -> anyhow::Result<Vec<FileManifest>> {
        let mut rx = Self::scan_directory_batches(
            Arc::clone(fs),
            conn_id.to_string(),
            base_path.to_string(),
            cancel.clone(),
        );
        let mut results = Vec::new();
        while let Some(batch) = rx.recv().await {
            results.extend(batch?);
        }
        Ok(results)
    }

    #[allow(clippy::too_many_arguments)]
    async fn scan_recursive_batched(
        fs: &Arc<dyn FileSystem>,
        conn_id: &str,
        base_path: &str,
        rel_path: &str,
        depth: usize,
        cancel: &CancellationToken,
        tx: &mpsc::Sender<ManifestBatch>,
        batch: &mut Vec<FileManifest>,
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
        while let Some(res) = stream.next().await {
            if cancel.is_cancelled() || *count >= MAX_ENTRIES {
                return Ok(());
            }

            let entry = res?;
            let entry_rel = if rel_path.is_empty() {
                entry.name.clone()
            } else {
                format!("{}/{}", rel_path, entry.name)
            };
            let entry_vfs = VfsPath::new(conn_id, &entry.path)?;
            let meta = fs.stat(&entry_vfs).await?;

            batch.push(FileManifest {
                path: entry_rel.clone(),
                kind: meta.kind,
                size: meta.size,
                modified_at: meta.modified_at,
                content_hash: None,
                etag: Some(meta.etag),
            });
            *count += 1;

            if batch.len() >= SCAN_BATCH_SIZE {
                let ready = std::mem::replace(batch, Vec::with_capacity(SCAN_BATCH_SIZE));
                if tx.send(Ok(ready)).await.is_err() {
                    return Ok(());
                }
            }

            if meta.kind == FileKind::Directory {
                Box::pin(Self::scan_recursive_batched(
                    fs,
                    conn_id,
                    base_path,
                    &entry_rel,
                    depth + 1,
                    cancel,
                    tx,
                    batch,
                    count,
                ))
                .await?;
            }
        }

        Ok(())
    }
}
