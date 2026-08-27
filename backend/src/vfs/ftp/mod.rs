use crate::domain::{Capabilities, FileEntry, FileKind, FileMetadata, VfsPath};
use crate::errors::VfsError;
use crate::vfs::traits::{AsyncReadBox, FileSystem};
use async_trait::async_trait;
use chrono::Utc;
use std::io::Cursor;
use std::time::Instant;
use suppaftp::tokio::AsyncFtpStream;

#[allow(dead_code)]
pub struct FtpFileSystem {
    connection_id: String,
    host: String,
    port: u16,
    username: Option<String>,
    password: Option<String>,
    is_secure: bool,
    base_path: String,
}

impl FtpFileSystem {
    pub fn new(
        connection_id: impl Into<String>,
        host: impl Into<String>,
        port: u16,
        username: Option<String>,
        password: Option<String>,
        is_secure: bool,
        base_path: impl Into<String>,
    ) -> Self {
        Self {
            connection_id: connection_id.into(),
            host: host.into(),
            port,
            username,
            password,
            is_secure,
            base_path: base_path.into(),
        }
    }

    /// Resolve remote path by prepending base_path with component containment
    fn resolve_remote_path(&self, vfs_path: &VfsPath) -> String {
        let clean_base = self.base_path.trim_end_matches('/');
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
        let clean_vfs = components.join("/");
        if clean_vfs.is_empty() {
            if clean_base.is_empty() {
                "/".to_string()
            } else {
                clean_base.to_string()
            }
        } else if clean_base.is_empty() {
            format!("/{}", clean_vfs)
        } else {
            format!("{}/{}", clean_base, clean_vfs)
        }
    }

    /// Establish an authenticated FTP connection
    async fn connect_ftp(&self) -> Result<AsyncFtpStream, VfsError> {
        let addr = format!("{}:{}", self.host, self.port);
        let mut ftp = AsyncFtpStream::connect(&addr).await.map_err(|e| {
            VfsError::ConnectionError(format!("Failed to connect to FTP host {}: {}", addr, e))
        })?;

        let user = self.username.as_deref().unwrap_or("anonymous");
        let pass = self.password.as_deref().unwrap_or("anonymous@example.com");

        ftp.login(user, pass).await.map_err(|e| {
            VfsError::ConnectionError(format!("FTP authentication failed for user {}: {}", user, e))
        })?;

        Ok(ftp)
    }

    /// Test connection latency to the remote FTP/FTPS server
    pub async fn test_connection(&self) -> Result<u64, VfsError> {
        let start = Instant::now();
        let mut ftp = self.connect_ftp().await?;
        let _ = ftp.pwd().await;
        let _ = ftp.quit().await;
        let elapsed = start.elapsed().as_millis() as u64;
        Ok(elapsed)
    }

    /// Parse Unix or Windows style FTP LIST output line
    fn parse_ftp_entry(line: &str, parent_vfs: &VfsPath) -> Option<FileEntry> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 9 {
            let is_dir = parts[0].starts_with('d');
            let is_link = parts[0].starts_with('l');
            let size: Option<u64> = parts[4].parse().ok();
            let name = parts[8..].join(" ");

            if name == "." || name == ".." {
                return None;
            }

            let kind = if is_dir {
                FileKind::Directory
            } else if is_link {
                FileKind::Symlink
            } else {
                FileKind::File
            };

            let entry_path = parent_vfs.join(&name).path;
            let is_hidden = name.starts_with('.');
            let perms = Some(parts[0].to_string());

            return Some(FileEntry {
                name,
                path: entry_path,
                kind,
                size: if is_dir { None } else { size },
                modified_at: Some(Utc::now()),
                permissions: perms,
                mime_type: None,
                is_hidden,
                symlink_target: None,
            });
        }

        let clean_name = trimmed.to_string();
        if clean_name == "." || clean_name == ".." {
            return None;
        }

        let is_dir = clean_name.ends_with('/');
        let name = clean_name.trim_end_matches('/').to_string();
        let entry_path = parent_vfs.join(&name).path;

        Some(FileEntry {
            name: name.clone(),
            path: entry_path,
            kind: if is_dir {
                FileKind::Directory
            } else {
                FileKind::File
            },
            size: None,
            modified_at: Some(Utc::now()),
            permissions: None,
            mime_type: None,
            is_hidden: name.starts_with('.'),
            symlink_target: None,
        })
    }
}

#[async_trait]
impl FileSystem for FtpFileSystem {
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
            copy: false,
            move_: true,
            upload: true,
            download: true,
            resume_upload: false,
            resume_download: true,
            atomic_write: false,
            atomic_rename: true,
            server_side_copy: false,
            symlink: false,
            permissions: false,
            watch: false,
            checksum: false,
        }
    }

    async fn list(&self, path: &VfsPath) -> Result<Vec<FileEntry>, VfsError> {
        let remote_path = self.resolve_remote_path(path);
        tracing::debug!("Listing remote FTP path: {}", remote_path);

        let mut ftp = self.connect_ftp().await?;
        let lines = ftp.list(Some(&remote_path)).await.map_err(|e| {
            VfsError::ConnectionError(format!("FTP list command failed on '{}': {}", remote_path, e))
        })?;

        let _ = ftp.quit().await;

        let mut entries = Vec::new();
        for line in lines {
            if let Some(entry) = Self::parse_ftp_entry(&line, path) {
                entries.push(entry);
            }
        }

        Ok(entries)
    }

    async fn stat(&self, path: &VfsPath) -> Result<FileMetadata, VfsError> {
        if path.is_root() {
            return Ok(FileMetadata {
                name: "root".to_string(),
                path: "/".to_string(),
                kind: FileKind::Directory,
                size: 0,
                modified_at: None,
                created_at: None,
                permissions: Some("0755".to_string()),
                mime_type: None,
                etag: "\"ftp-root\"".to_string(),
                is_readonly: false,
                is_hidden: false,
                symlink_target: None,
            });
        }

        let remote_path = self.resolve_remote_path(path);
        let mut ftp = self.connect_ftp().await?;

        // 1. Try MDTM (modification time) and SIZE to check if it's a file
        let mtime_res = ftp.mdtm(&remote_path).await;
        let size_res = ftp.size(&remote_path).await;

        let (is_dir, size, mtime) = if let Ok(sz) = size_res {
            let mtime_opt = mtime_res.ok().map(|dt| {
                chrono::DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)
            });
            (false, sz as u64, mtime_opt)
        } else if let Ok(dt) = mtime_res {
            let mtime_utc = chrono::DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc);
            (false, 0u64, Some(mtime_utc))
        } else {
            // Check if directory by attempting CWD
            let is_directory = ftp.cwd(&remote_path).await.is_ok();
            (is_directory, 0u64, None)
        };

        let _ = ftp.quit().await;

        let name = path.file_name().unwrap_or("entry").to_string();

        // 2. Deterministic ETag based on path, size, and modified time (never now())
        let mtime_ts = mtime.map(|m| m.timestamp()).unwrap_or(0);
        let etag = if is_dir {
            format!("\"ftp-dir-{}\"", path.path)
        } else {
            format!("\"ftp-{}-{}-{}\"", path.path, size, mtime_ts)
        };

        Ok(FileMetadata {
            name: name.clone(),
            path: path.path.clone(),
            kind: if is_dir {
                FileKind::Directory
            } else {
                FileKind::File
            },
            size,
            modified_at: mtime,
            created_at: None,
            permissions: Some(if is_dir { "0755" } else { "0644" }.to_string()),
            mime_type: if is_dir {
                None
            } else {
                Some(mime_guess::from_path(&name).first_or_octet_stream().to_string())
            },
            etag,
            is_readonly: false,
            is_hidden: name.starts_with('.'),
            symlink_target: None,
        })
    }

    async fn read_stream(&self, path: &VfsPath) -> Result<AsyncReadBox, VfsError> {
        let remote_path = self.resolve_remote_path(path);
        let mut ftp = self.connect_ftp().await?;

        let mut data_stream = ftp.retr_as_stream(&remote_path).await.map_err(|e| {
            VfsError::ConnectionError(format!("Failed to retrieve FTP file '{}': {}", remote_path, e))
        })?;

        let (pipe_reader, mut pipe_writer) = tokio::io::duplex(64 * 1024);

        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buf = [0u8; 64 * 1024];
            loop {
                match data_stream.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        if pipe_writer.write_all(&buf[..n]).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            let _ = pipe_writer.flush().await;
            let _ = ftp.finalize_retr_stream(data_stream).await;
            let _ = ftp.quit().await;
        });

        Ok(Box::new(pipe_reader))
    }

    async fn write_stream(&self, path: &VfsPath, mut input: AsyncReadBox) -> Result<(), VfsError> {
        let remote_path = self.resolve_remote_path(path);
        let mut ftp = self.connect_ftp().await?;

        ftp.put_file(&remote_path, &mut input).await.map_err(|e| {
            VfsError::ConnectionError(format!("Failed to upload FTP file '{}': {}", remote_path, e))
        })?;

        let _ = ftp.quit().await;
        Ok(())
    }

    async fn create_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        let remote_path = self.resolve_remote_path(path);
        let mut ftp = self.connect_ftp().await?;
        let mut cursor = Cursor::new(Vec::<u8>::new());

        ftp.put_file(&remote_path, &mut cursor).await.map_err(|e| {
            VfsError::ConnectionError(format!("Failed to create FTP file '{}': {}", remote_path, e))
        })?;

        let _ = ftp.quit().await;
        Ok(())
    }

    async fn create_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        let remote_path = self.resolve_remote_path(path);
        let mut ftp = self.connect_ftp().await?;

        ftp.mkdir(&remote_path).await.map_err(|e| {
            VfsError::ConnectionError(format!("Failed to create FTP directory '{}': {}", remote_path, e))
        })?;

        let _ = ftp.quit().await;
        Ok(())
    }

    async fn delete(&self, path: &VfsPath) -> Result<(), VfsError> {
        let remote_path = self.resolve_remote_path(path);
        let mut ftp = self.connect_ftp().await?;

        let res = ftp.rm(&remote_path).await;
        if res.is_err() {
            // Try removing directory
            ftp.rmdir(&remote_path).await.map_err(|e| {
                VfsError::ConnectionError(format!("Failed to delete FTP path '{}': {}", remote_path, e))
            })?;
        }

        let _ = ftp.quit().await;
        Ok(())
    }

    async fn rename(&self, from: &VfsPath, to: &VfsPath) -> Result<(), VfsError> {
        let from_remote = self.resolve_remote_path(from);
        let to_remote = self.resolve_remote_path(to);

        let mut ftp = self.connect_ftp().await?;
        ftp.rename(&from_remote, &to_remote).await.map_err(|e| {
            VfsError::ConnectionError(format!("Failed to rename FTP path from '{}' to '{}': {}", from_remote, to_remote, e))
        })?;

        let _ = ftp.quit().await;
        Ok(())
    }

    async fn copy(&self, from: &VfsPath, to: &VfsPath) -> Result<(), VfsError> {
        let stream = self.read_stream(from).await?;
        self.write_stream(to, stream).await?;
        Ok(())
    }
}
