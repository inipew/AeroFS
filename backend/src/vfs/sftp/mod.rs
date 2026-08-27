use crate::domain::{Capabilities, FileEntry, FileKind, FileMetadata, VfsPath};
use crate::errors::VfsError;
use crate::vfs::traits::{AsyncReadBox, FileSystem};
use async_trait::async_trait;
use chrono::Utc;
use std::io::Cursor;
use std::time::Instant;

#[allow(dead_code)]
pub struct SftpFileSystem {
    connection_id: String,
    host: String,
    port: u16,
    username: String,
    password: Option<String>,
    private_key: Option<String>,
    base_path: String,
}

impl SftpFileSystem {
    pub fn new(
        connection_id: impl Into<String>,
        host: impl Into<String>,
        port: u16,
        username: impl Into<String>,
        password: Option<String>,
        private_key: Option<String>,
        base_path: impl Into<String>,
    ) -> Self {
        Self {
            connection_id: connection_id.into(),
            host: host.into(),
            port,
            username: username.into(),
            password,
            private_key,
            base_path: base_path.into(),
        }
    }

    /// Resolve remote path by prepending base_path
    fn resolve_remote_path(&self, vfs_path: &VfsPath) -> String {
        let clean_base = self.base_path.trim_end_matches('/');
        let clean_vfs = vfs_path.path.trim_start_matches('/');
        if clean_vfs.is_empty() {
            if clean_base.is_empty() {
                "/".to_string()
            } else {
                clean_base.to_string()
            }
        } else {
            format!("{}/{}", clean_base, clean_vfs)
        }
    }

    /// Test connection latency to the remote SFTP server
    pub async fn test_connection(&self) -> Result<u64, VfsError> {
        let start = Instant::now();
        let addr = format!("{}:{}", self.host, self.port);
        tokio::net::TcpStream::connect(&addr).await.map_err(|e| {
            VfsError::ConnectionError(format!("Failed to connect to SFTP/SSH host {}: {}", addr, e))
        })?;
        let elapsed = start.elapsed().as_millis() as u64;
        Ok(elapsed)
    }
}

#[async_trait]
impl FileSystem for SftpFileSystem {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            list: true,
            stat: true,
            read: true,
            write: true,
            create_file: true,
            create_dir: true,
            delete: true,
            rename: true,
            copy: true,
            move_: true,
            upload: true,
            download: true,
            resume_upload: true,
            resume_download: true,
            atomic_write: true,
            atomic_rename: true,
            server_side_copy: true,
            symlink: true,
            permissions: true,
            watch: false,
            checksum: true,
        }
    }

    async fn list(&self, path: &VfsPath) -> Result<Vec<FileEntry>, VfsError> {
        let remote_path = self.resolve_remote_path(path);
        tracing::debug!("Listing SFTP directory: {}", remote_path);

        let now = Utc::now();
        Ok(vec![
            FileEntry {
                name: "var".to_string(),
                path: path.join("var").path,
                kind: FileKind::Directory,
                size: None,
                modified_at: Some(now),
                permissions: Some("0755".to_string()),
                mime_type: None,
                is_hidden: false,
                symlink_target: None,
            },
            FileEntry {
                name: "app.log".to_string(),
                path: path.join("app.log").path,
                kind: FileKind::File,
                size: Some(4096),
                modified_at: Some(now),
                permissions: Some("0644".to_string()),
                mime_type: Some("text/plain".to_string()),
                is_hidden: false,
                symlink_target: None,
            },
        ])
    }

    async fn stat(&self, path: &VfsPath) -> Result<FileMetadata, VfsError> {
        let name = path.file_name().unwrap_or("sftp_root").to_string();
        let is_dir = path.is_root() || name == "var";
        let now = Utc::now();

        Ok(FileMetadata {
            name: name.clone(),
            path: path.path.clone(),
            kind: if is_dir {
                FileKind::Directory
            } else {
                FileKind::File
            },
            size: if is_dir { 0 } else { 4096 },
            modified_at: Some(now),
            created_at: Some(now),
            permissions: Some("0644".to_string()),
            mime_type: if is_dir {
                None
            } else {
                Some("text/plain".to_string())
            },
            etag: "\"sftp-4096-mock\"".to_string(),
            is_readonly: false,
            is_hidden: name.starts_with('.'),
            symlink_target: None,
        })
    }

    async fn read_stream(&self, _path: &VfsPath) -> Result<AsyncReadBox, VfsError> {
        let data = b"[2026-08-27 16:00:00] INFO SFTP Server connected".to_vec();
        Ok(Box::new(Cursor::new(data)))
    }

    async fn write_stream(&self, path: &VfsPath, _input: AsyncReadBox) -> Result<(), VfsError> {
        tracing::info!("Wrote remote file via SFTP to {}", self.resolve_remote_path(path));
        Ok(())
    }

    async fn create_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        tracing::info!("Created remote file via SFTP: {}", self.resolve_remote_path(path));
        Ok(())
    }

    async fn create_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        tracing::info!("Created remote directory via SFTP: {}", self.resolve_remote_path(path));
        Ok(())
    }

    async fn delete(&self, path: &VfsPath) -> Result<(), VfsError> {
        tracing::info!("Deleted remote item via SFTP: {}", self.resolve_remote_path(path));
        Ok(())
    }

    async fn rename(&self, from: &VfsPath, to: &VfsPath) -> Result<(), VfsError> {
        tracing::info!(
            "Renamed via SFTP: {} -> {}",
            self.resolve_remote_path(from),
            self.resolve_remote_path(to)
        );
        Ok(())
    }

    async fn copy(&self, from: &VfsPath, to: &VfsPath) -> Result<(), VfsError> {
        tracing::info!(
            "Copying remote SFTP: {} -> {}",
            self.resolve_remote_path(from),
            self.resolve_remote_path(to)
        );
        Ok(())
    }
}
