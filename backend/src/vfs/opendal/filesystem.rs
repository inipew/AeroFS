use super::capabilities::map_opendal_capabilities_for_scheme;
use super::error::map_opendal_error;
use super::lister::{create_opendal_stream, FileStreamBox};
use super::metadata::map_opendal_metadata;
use crate::domain::{Capabilities, FileKind, FileMetadata, VfsPath};
use crate::errors::VfsError;
use crate::vfs::traits::{AsyncReadBox, FileSystem, PresignSupport};
use async_trait::async_trait;
use futures::StreamExt;
use opendal::{ErrorKind, Operator};
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio_util::bytes::BytesMut;

/// Universal OpenDAL-backed VFS FileSystem implementation
pub struct OpenDalFileSystem {
    connection_id: String,
    operator: Operator,
    capabilities: Capabilities,
    local_root: Option<std::path::PathBuf>,
}

impl OpenDalFileSystem {
    pub fn connection_id(&self) -> &str {
        &self.connection_id
    }

    pub fn operator(&self) -> &Operator {
        &self.operator
    }

    pub fn new(connection_id: impl Into<String>, operator: Operator) -> Self {
        let conn_id = connection_id.into();
        let raw_cap = operator.info().capability();
        let scheme = operator.info().scheme();
        let capabilities = map_opendal_capabilities_for_scheme(raw_cap, scheme);

        Self {
            connection_id: conn_id,
            operator,
            capabilities,
            local_root: None,
        }
    }

    pub fn new_local(
        connection_id: impl Into<String>,
        operator: Operator,
        local_root: impl Into<std::path::PathBuf>,
    ) -> Self {
        let conn_id = connection_id.into();
        let raw_cap = operator.info().capability();
        let scheme = operator.info().scheme();
        let capabilities = map_opendal_capabilities_for_scheme(raw_cap, scheme);

        Self {
            connection_id: conn_id,
            operator,
            capabilities,
            local_root: Some(local_root.into()),
        }
    }

    pub fn with_local_root(mut self, root: impl Into<std::path::PathBuf>) -> Self {
        self.local_root = Some(root.into());
        self
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
            local_root: None,
        }
    }

    /// Strict path validation: converts VfsPath to OpenDAL relative path.
    /// Traversal attempts (`..`) and invalid prefixes are strictly REJECTED.
    fn to_operator_path(&self, vfs_path: &VfsPath) -> Result<String, VfsError> {
        let path_str = &vfs_path.path;
        for comp in std::path::Path::new(path_str).components() {
            match comp {
                std::path::Component::ParentDir => {
                    return Err(VfsError::InvalidPath(format!(
                        "Path traversal attempt rejected in '{}'",
                        path_str
                    )));
                }
                std::path::Component::Prefix(_) => {
                    return Err(VfsError::InvalidPath(format!(
                        "Drive prefix rejected in '{}'",
                        path_str
                    )));
                }
                _ => {}
            }
        }
        let clean = path_str.trim_start_matches('/').trim_end_matches('/');
        Ok(clean.to_string())
    }

    /// Convert VfsPath to directory path format with trailing slash
    fn to_operator_dir_path(&self, vfs_path: &VfsPath) -> Result<String, VfsError> {
        let clean = self.to_operator_path(vfs_path)?;
        if clean.is_empty() {
            Ok(String::new())
        } else {
            Ok(format!("{}/", clean))
        }
    }
}

#[async_trait]
impl PresignSupport for OpenDalFileSystem {
    async fn presign_read_url(&self, path: &VfsPath, expire: Duration) -> Result<String, VfsError> {
        let op_path = self.to_operator_path(path)?;
        let signed_req = self
            .operator
            .presign_read(&op_path, expire)
            .await
            .map_err(|e| {
                map_opendal_error(e, &format!("Presign read failed for '{}'", path.path))
            })?;
        Ok(signed_req.uri().to_string())
    }

    async fn presign_write_url(
        &self,
        path: &VfsPath,
        expire: Duration,
    ) -> Result<String, VfsError> {
        let op_path = self.to_operator_path(path)?;
        let signed_req = self
            .operator
            .presign_write(&op_path, expire)
            .await
            .map_err(|e| {
                map_opendal_error(e, &format!("Presign write failed for '{}'", path.path))
            })?;
        Ok(signed_req.uri().to_string())
    }
}

#[async_trait]
impl FileSystem for OpenDalFileSystem {
    fn capabilities(&self) -> Capabilities {
        self.capabilities.clone()
    }

    fn as_presign(&self) -> Option<&dyn PresignSupport> {
        if self.capabilities.presign_read || self.capabilities.presign_write {
            Some(self)
        } else {
            None
        }
    }

    #[tracing::instrument(skip(self), fields(conn = %self.connection_id, path = %path.path))]
    async fn list_stream(&self, path: &VfsPath) -> Result<FileStreamBox, VfsError> {
        let op_path = self.to_operator_dir_path(path)?;
        let list_target = if op_path.is_empty() { "/" } else { &op_path };
        create_opendal_stream(&self.operator, list_target, path, self.local_root.clone()).await
    }

    #[tracing::instrument(skip(self), fields(conn = %self.connection_id, path = %path.path))]
    async fn stat(&self, path: &VfsPath) -> Result<FileMetadata, VfsError> {
        // Honest root stat handling: propagate connection/auth errors, only fallback on NotFound or Unsupported
        if path.is_root() {
            let real_meta = match self.operator.stat("/").await {
                Ok(m) => Some(m),
                Err(e) if e.kind() == ErrorKind::NotFound || e.kind() == ErrorKind::Unsupported => {
                    None
                }
                Err(e) => return Err(map_opendal_error(e, "Failed to stat root directory")),
            };
            let size = real_meta.as_ref().map(|m| m.content_length()).unwrap_or(0);
            let modified_at = real_meta
                .as_ref()
                .and_then(|m| m.last_modified())
                .map(|dt| {
                    let st: std::time::SystemTime = dt.into();
                    chrono::DateTime::<chrono::Utc>::from(st)
                });

            let mut permissions = None;
            #[cfg(unix)]
            if let Some(ref root) = self.local_root {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(sym_meta) = std::fs::symlink_metadata(root) {
                    let mode = sym_meta.permissions().mode() & 0o7777;
                    permissions = Some(format!("{:04o}", mode));
                }
            }

            return Ok(FileMetadata {
                name: "root".to_string(),
                path: "/".to_string(),
                kind: FileKind::Directory,
                size,
                modified_at,
                created_at: None,
                permissions,
                mime_type: None,
                etag: format!("\"od-root-{}\"", self.connection_id),
                is_readonly: false,
                is_hidden: false,
                symlink_target: None,
            });
        }

        let op_path = self.to_operator_path(path)?;
        let meta = self
            .operator
            .stat(&op_path)
            .await
            .map_err(|e| map_opendal_error(e, &format!("Failed to stat '{}'", path.path)))?;

        let mut res = map_opendal_metadata(&meta, path, false);
        #[cfg(unix)]
        if let Some(ref root) = self.local_root {
            use std::os::unix::fs::PermissionsExt;
            let abs_path = root.join(path.path.trim_start_matches('/'));
            if let Ok(sym_meta) = std::fs::symlink_metadata(&abs_path) {
                let mode = sym_meta.permissions().mode() & 0o7777;
                res.permissions = Some(format!("{:04o}", mode));
            }
        }
        Ok(res)
    }

    #[tracing::instrument(skip(self), fields(conn = %self.connection_id, path = %path.path))]
    async fn set_permissions(&self, path: &VfsPath, permissions: &str) -> Result<(), VfsError> {
        #[cfg(unix)]
        if let Some(ref root) = self.local_root {
            use std::os::unix::fs::PermissionsExt;
            let clean_perms = permissions.trim_start_matches('0');
            let mode = if clean_perms.is_empty() {
                0o000
            } else {
                u32::from_str_radix(clean_perms, 8).map_err(|e| {
                    VfsError::InvalidPath(format!("Invalid octal mode '{}': {}", permissions, e))
                })?
            };
            let abs_path = root.join(path.path.trim_start_matches('/'));
            std::fs::set_permissions(&abs_path, std::fs::Permissions::from_mode(mode))
                .map_err(|e| VfsError::IoError(format!("Failed to chmod {:?}: {}", abs_path, e)))?;
            return Ok(());
        }

        let _ = path;
        let _ = permissions;
        Ok(())
    }

    #[tracing::instrument(skip(self), fields(conn = %self.connection_id, path = %path.path))]
    async fn read_stream(&self, path: &VfsPath) -> Result<AsyncReadBox, VfsError> {
        let op_path = self.to_operator_path(path)?;
        let reader = self.operator.reader(&op_path).await.map_err(|e| {
            map_opendal_error(e, &format!("Failed to open reader for '{}'", path.path))
        })?;

        let stream = reader
            .into_bytes_stream(..)
            .await
            .map_err(|e| {
                map_opendal_error(
                    e,
                    &format!("Failed to open bytes stream for '{}'", path.path),
                )
            })?
            .map(|res| res.map_err(|e| std::io::Error::other(e.to_string())));

        let async_reader = tokio_util::io::StreamReader::new(stream);
        Ok(Box::new(async_reader))
    }

    #[tracing::instrument(skip(self), fields(conn = %self.connection_id, path = %path.path, offset = %offset, length = %length))]
    async fn read_range(
        &self,
        path: &VfsPath,
        offset: u64,
        length: u64,
    ) -> Result<AsyncReadBox, VfsError> {
        let op_path = self.to_operator_path(path)?;
        if length == 0 {
            return Ok(Box::new(std::io::Cursor::new(Vec::new())));
        }

        // OpenDAL Native: Range read directly without mandatory pre-stat HEAD request
        let reader = self.operator.reader(&op_path).await.map_err(|e| {
            map_opendal_error(
                e,
                &format!("Failed to open range reader for '{}'", path.path),
            )
        })?;

        let stream = reader
            .into_bytes_stream(offset..)
            .await
            .map_err(|e| {
                map_opendal_error(
                    e,
                    &format!("Failed to open bytes range stream for '{}'", path.path),
                )
            })?
            .map(|res| res.map_err(|e| std::io::Error::other(e.to_string())));

        let async_reader = tokio_util::io::StreamReader::new(stream).take(length);
        Ok(Box::new(async_reader))
    }

    #[tracing::instrument(skip(self, input), fields(conn = %self.connection_id, path = %path.path))]
    async fn write_stream(&self, path: &VfsPath, mut input: AsyncReadBox) -> Result<(), VfsError> {
        let op_path = self.to_operator_path(path)?;
        let mut writer = self.operator.writer(&op_path).await.map_err(|e| {
            map_opendal_error(e, &format!("Failed to open writer for '{}'", path.path))
        })?;

        let mut buf = BytesMut::with_capacity(64 * 1024);
        loop {
            buf.clear();
            buf.resize(64 * 1024, 0);
            let n = input
                .read(&mut buf)
                .await
                .map_err(|e| VfsError::IoError(e.to_string()))?;
            if n == 0 {
                break;
            }
            buf.truncate(n);
            writer.write(buf.split().freeze()).await.map_err(|e| {
                map_opendal_error(e, &format!("Write chunk failed for '{}'", path.path))
            })?;
        }

        writer.close().await.map_err(|e| {
            map_opendal_error(e, &format!("Failed to finalize write for '{}'", path.path))
        })?;

        Ok(())
    }

    #[tracing::instrument(skip(self), fields(conn = %self.connection_id, path = %path.path))]
    async fn create_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        let op_path = self.to_operator_path(path)?;
        self.operator
            .write(&op_path, Vec::<u8>::new())
            .await
            .map_err(|e| map_opendal_error(e, &format!("Failed to create file '{}'", path.path)))?;
        Ok(())
    }

    #[tracing::instrument(skip(self), fields(conn = %self.connection_id, path = %path.path))]
    async fn create_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        let op_path = self.to_operator_dir_path(path)?;
        self.operator
            .create_dir(&op_path)
            .await
            .map_err(|e| map_opendal_error(e, &format!("Failed to create dir '{}'", path.path)))?;
        Ok(())
    }

    #[tracing::instrument(skip(self), fields(conn = %self.connection_id, path = %path.path))]
    async fn delete(&self, path: &VfsPath) -> Result<(), VfsError> {
        let op_path = self.to_operator_path(path)?;

        // 1. Try standard delete
        let delete_res = self.operator.delete(&op_path).await;
        match delete_res {
            Ok(_) => Ok(()),
            Err(e) => {
                // 2. If standard delete failed (e.g. FTP directory requiring trailing slash for rmdir)
                let dir_op_path = format!("{}/", op_path);
                if self.operator.delete(&dir_op_path).await.is_ok() {
                    return Ok(());
                }

                // 3. If non-empty directory, recursively delete children using streaming list_stream
                if let Ok(mut stream) = self.list_stream(path).await {
                    let mut has_entries = false;
                    while let Some(entry_res) = stream.next().await {
                        if let Ok(entry) = entry_res {
                            has_entries = true;
                            if let Ok(child_vfs) = VfsPath::new(&self.connection_id, &entry.path) {
                                let _ = Box::pin(self.delete(&child_vfs)).await;
                            }
                        }
                    }

                    if has_entries {
                        if self.operator.delete(&dir_op_path).await.is_ok() {
                            return Ok(());
                        }
                        if self.operator.delete(&op_path).await.is_ok() {
                            return Ok(());
                        }
                    }
                }

                Err(map_opendal_error(
                    e,
                    &format!("Failed to delete '{}'", path.path),
                ))
            }
        }
    }

    #[tracing::instrument(skip(self), fields(conn = %self.connection_id, from = %from.path, to = %to.path))]
    async fn rename(&self, from: &VfsPath, to: &VfsPath) -> Result<(), VfsError> {
        let from_path = self.to_operator_path(from)?;
        let to_path = self.to_operator_path(to)?;

        match self.operator.rename(&from_path, &to_path).await {
            Ok(_) => Ok(()),
            Err(e) => Err(map_opendal_error(
                e,
                &format!("Failed to rename '{}' to '{}'", from.path, to.path),
            )),
        }
    }

    #[tracing::instrument(skip(self), fields(conn = %self.connection_id, from = %from.path, to = %to.path))]
    async fn copy(&self, from: &VfsPath, to: &VfsPath) -> Result<(), VfsError> {
        let meta = self.stat(from).await?;
        let existing_dst_perms = if self.capabilities().permissions {
            self.stat(to).await.ok().and_then(|m| m.permissions)
        } else {
            None
        };

        if meta.kind == FileKind::Directory {
            self.create_dir(to).await?;
            let mut stream = self.list_stream(from).await?;
            while let Some(res) = stream.next().await {
                let entry = res?;
                let child_name = entry.name;
                let child_from = from.join(&child_name)?;
                let child_to = to.join(&child_name)?;
                Box::pin(self.copy(&child_from, &child_to)).await?;
            }

            if let Some(ref perms) = meta.permissions {
                let _ = self.set_permissions(to, perms).await;
            }
        } else {
            let from_path = self.to_operator_path(from)?;
            let to_path = self.to_operator_path(to)?;
            self.operator
                .copy(&from_path, &to_path)
                .await
                .map_err(|e| {
                    map_opendal_error(
                        e,
                        &format!("Failed to copy '{}' to '{}'", from.path, to.path),
                    )
                })?;

            if self.capabilities().permissions {
                let perms_to_apply = existing_dst_perms.or(meta.permissions);
                if let Some(ref perms) = perms_to_apply {
                    let _ = self.set_permissions(to, perms).await;
                }
            }
        }
        Ok(())
    }
}
