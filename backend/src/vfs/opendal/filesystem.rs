use super::capabilities::map_opendal_capabilities;
use super::error::map_opendal_error;
use super::metadata::{map_opendal_entry, map_opendal_metadata};
use crate::domain::{Capabilities, FileEntry, FileKind, FileMetadata, VfsPath};
use crate::errors::VfsError;
use crate::vfs::traits::{AsyncReadBox, FileSystem};
use async_trait::async_trait;
use chrono::Utc;
use futures::StreamExt;
use opendal::Operator;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Universal OpenDAL-backed VFS FileSystem implementation
pub struct OpenDalFileSystem {
    #[allow(dead_code)]
    connection_id: String,
    operator: Operator,
    capabilities: Capabilities,
}

impl OpenDalFileSystem {
    pub fn connection_id(&self) -> &str {
        &self.connection_id
    }

    pub fn new(connection_id: impl Into<String>, operator: Operator) -> Self {
        let conn_id = connection_id.into();
        let raw_cap = operator.info().capability();
        let capabilities = map_opendal_capabilities(raw_cap);

        Self {
            connection_id: conn_id,
            operator,
            capabilities,
        }
    }

    /// Custom constructor allowing capability overrides
    pub fn new_with_capabilities(
        connection_id: impl Into<String>,
        operator: Operator,
        capabilities: Capabilities,
    ) -> Self {
        Self {
            connection_id: connection_id.into(),
            operator,
            capabilities,
        }
    }

    /// Normalize VfsPath into OpenDAL-relative path format with containment
    fn normalize_path(&self, vfs_path: &VfsPath) -> String {
        let mut components = Vec::new();
        for comp in std::path::Path::new(&vfs_path.path).components() {
            match comp {
                std::path::Component::Normal(c) => components.push(c.to_string_lossy().to_string()),
                std::path::Component::ParentDir => {
                    components.pop();
                }
                _ => {}
            }
        }
        components.join("/")
    }

    /// Normalize directory path with trailing slash for OpenDAL directory semantics
    fn normalize_dir_path(&self, vfs_path: &VfsPath) -> String {
        let clean = self.normalize_path(vfs_path);
        if clean.is_empty() {
            String::new()
        } else if clean.ends_with('/') {
            clean
        } else {
            format!("{}/", clean)
        }
    }
}

#[async_trait]
impl FileSystem for OpenDalFileSystem {
    fn capabilities(&self) -> Capabilities {
        self.capabilities.clone()
    }

    async fn list(&self, path: &VfsPath) -> Result<Vec<FileEntry>, VfsError> {
        let op_path = self.normalize_dir_path(path);
        let list_target = if op_path.is_empty() { "/" } else { &op_path };

        let entries = self
            .operator
            .list(list_target)
            .await
            .map_err(|e| map_opendal_error(e, &format!("Failed to list '{}'", path.path)))?;

        let mut results = Vec::new();
        for entry in entries {
            if let Some(mapped) = map_opendal_entry(&entry, path) {
                results.push(mapped);
            }
        }

        Ok(results)
    }

    async fn stat(&self, path: &VfsPath) -> Result<FileMetadata, VfsError> {
        if path.is_root() {
            let now = Utc::now();
            return Ok(FileMetadata {
                name: "root".to_string(),
                path: "/".to_string(),
                kind: FileKind::Directory,
                size: 0,
                modified_at: Some(now),
                created_at: Some(now),
                permissions: Some("0755".to_string()),
                mime_type: None,
                etag: "\"od-root\"".to_string(),
                is_readonly: false,
                is_hidden: false,
                symlink_target: None,
            });
        }

        let op_path = self.normalize_path(path);
        let meta = self
            .operator
            .stat(&op_path)
            .await
            .map_err(|e| map_opendal_error(e, &format!("Failed to stat '{}'", path.path)))?;

        Ok(map_opendal_metadata(&meta, path, false))
    }

    async fn read_stream(&self, path: &VfsPath) -> Result<AsyncReadBox, VfsError> {
        let op_path = self.normalize_path(path);
        let reader = self
            .operator
            .reader(&op_path)
            .await
            .map_err(|e| map_opendal_error(e, &format!("Failed to open reader for '{}'", path.path)))?;

        // 64 KB Bounded Duplex Stream Pipe (O(1) memory consumption)
        let (pipe_reader, mut pipe_writer) = tokio::io::duplex(64 * 1024);

        tokio::spawn(async move {
            if let Ok(mut stream) = reader.into_bytes_stream(..).await {
                while let Some(chunk_res) = stream.next().await {
                    match chunk_res {
                        Ok(chunk) => {
                            if pipe_writer.write_all(&chunk).await.is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
            let _ = pipe_writer.flush().await;
        });

        Ok(Box::new(pipe_reader))
    }

    async fn write_stream(&self, path: &VfsPath, mut input: AsyncReadBox) -> Result<(), VfsError> {
        let op_path = self.normalize_path(path);
        let mut writer = self
            .operator
            .writer(&op_path)
            .await
            .map_err(|e| map_opendal_error(e, &format!("Failed to open writer for '{}'", path.path)))?;

        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = input
                .read(&mut buf)
                .await
                .map_err(|e| VfsError::IoError(e.to_string()))?;
            if n == 0 {
                break;
            }
            let bytes = axum::body::Bytes::copy_from_slice(&buf[..n]);
            writer
                .write(bytes)
                .await
                .map_err(|e| map_opendal_error(e, &format!("Write chunk failed for '{}'", path.path)))?;
        }

        writer
            .close()
            .await
            .map_err(|e| map_opendal_error(e, &format!("Failed to finalize write for '{}'", path.path)))?;

        Ok(())
    }

    async fn create_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        let op_path = self.normalize_path(path);
        self.operator
            .write(&op_path, Vec::<u8>::new())
            .await
            .map_err(|e| map_opendal_error(e, &format!("Failed to create file '{}'", path.path)))?;
        Ok(())
    }

    async fn create_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        let op_path = self.normalize_dir_path(path);
        self.operator
            .create_dir(&op_path)
            .await
            .map_err(|e| map_opendal_error(e, &format!("Failed to create dir '{}'", path.path)))?;
        Ok(())
    }

    async fn delete(&self, path: &VfsPath) -> Result<(), VfsError> {
        let op_path = self.normalize_path(path);
        let res = self.operator.delete_with(&op_path).recursive(true).await;
        if let Err(e) = res {
            self.operator
                .delete(&op_path)
                .await
                .map_err(|err| map_opendal_error(err, &format!("Failed to delete '{}' (fallback: {})", path.path, e)))?;
        }
        Ok(())
    }

    async fn rename(&self, from: &VfsPath, to: &VfsPath) -> Result<(), VfsError> {
        let from_path = self.normalize_path(from);
        let to_path = self.normalize_path(to);

        self.operator
            .rename(&from_path, &to_path)
            .await
            .map_err(|e| {
                map_opendal_error(
                    e,
                    &format!("Failed to rename '{}' to '{}'", from.path, to.path),
                )
            })?;
        Ok(())
    }

    async fn copy(&self, from: &VfsPath, to: &VfsPath) -> Result<(), VfsError> {
        let from_path = self.normalize_path(from);
        let to_path = self.normalize_path(to);

        // Try server-side/native copy first
        let res = self.operator.copy(&from_path, &to_path).await;
        if res.is_ok() {
            return Ok(());
        }

        // Fallback to streaming copy if native copy is unsupported
        let stream = self.read_stream(from).await?;
        self.write_stream(to, stream).await?;
        Ok(())
    }
}
