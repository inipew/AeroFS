use crate::domain::{Capabilities, FileEntry, FileKind, FileMetadata, VfsPath};
use crate::errors::{SecurityError, VfsError};
use crate::filesystem::SafePath;
use crate::vfs::traits::{AsyncReadBox, FileSystem};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;

pub struct LocalFileSystem {
    connection_id: String,
    root_dir: PathBuf,
    allow_symlinks_outside: bool,
}

impl LocalFileSystem {
    pub fn new(
        connection_id: impl Into<String>,
        root_dir: impl Into<PathBuf>,
        allow_symlinks_outside: bool,
    ) -> Self {
        let root = root_dir.into();
        Self {
            connection_id: connection_id.into(),
            root_dir: root,
            allow_symlinks_outside,
        }
    }

    fn resolve_safe(&self, path: &VfsPath) -> Result<SafePath, VfsError> {
        SafePath::from_vfs_path(&self.root_dir, path, self.allow_symlinks_outside).map_err(|e| {
            match e {
                SecurityError::PathTraversal(msg) => VfsError::PermissionDenied(msg),
                SecurityError::SymlinkEscape(msg) => VfsError::PermissionDenied(msg),
                SecurityError::InvalidPath(msg) => VfsError::NotFound(msg),
                SecurityError::AccessDenied(msg) => VfsError::PermissionDenied(msg),
                SecurityError::SsrfBlocked(msg) => VfsError::PermissionDenied(msg),
                SecurityError::NullByte => VfsError::InvalidPath("Null byte in path".into()),
            }
        })
    }

    fn calculate_etag(size: u64, modified_at: Option<SystemTime>) -> String {
        let ts = modified_at
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        format!("\"{:x}-{:x}\"", size, ts)
    }

    fn format_permissions(mode: u32) -> String {
        format!("{:04o}", mode & 0o7777)
    }
}

#[async_trait]
impl FileSystem for LocalFileSystem {
    fn capabilities(&self) -> Capabilities {
        Capabilities::local_default()
    }

    async fn list(&self, path: &VfsPath) -> Result<Vec<FileEntry>, VfsError> {
        let safe = self.resolve_safe(path)?;
        let abs_path = safe.absolute();

        if !abs_path.exists() {
            return Err(VfsError::NotFound(path.path.clone()));
        }

        if !abs_path.is_dir() {
            return Err(VfsError::NotADirectory(path.path.clone()));
        }

        let mut read_dir = fs::read_dir(abs_path).await.map_err(|e| {
            VfsError::IoError(format!("Failed to read directory {}: {}", path.path, e))
        })?;

        let mut entries = Vec::new();
        while let Some(entry) = read_dir.next_entry().await.map_err(|e| {
            VfsError::IoError(format!("Failed to iterate directory {}: {}", path.path, e))
        })? {
            let file_name = entry.file_name().to_string_lossy().to_string();
            let is_hidden = file_name.starts_with('.');
            let entry_path = entry.path();
            let metadata = entry.metadata().await.ok();

            let (kind, size, modified_at, permissions) = if let Some(meta) = metadata {
                let file_type = meta.file_type();
                let kind = if file_type.is_symlink() {
                    FileKind::Symlink
                } else if file_type.is_dir() {
                    FileKind::Directory
                } else {
                    FileKind::File
                };

                let size = if kind == FileKind::File {
                    Some(meta.len())
                } else {
                    None
                };

                let modified_at = meta.modified().ok().map(|t| DateTime::<Utc>::from(t));

                #[cfg(unix)]
                let permissions = {
                    use std::os::unix::fs::MetadataExt;
                    Some(Self::format_permissions(meta.mode()))
                };
                #[cfg(not(unix))]
                let permissions = None;

                (kind, size, modified_at, permissions)
            } else {
                (FileKind::File, None, None, None)
            };

            let mime_type = if kind == FileKind::File {
                mime_guess::from_path(&entry_path)
                    .first_raw()
                    .map(|s| s.to_string())
            } else {
                None
            };

            let symlink_target = if kind == FileKind::Symlink {
                fs::read_link(&entry_path)
                    .await
                    .ok()
                    .map(|p| p.to_string_lossy().to_string())
            } else {
                None
            };

            let entry_vfs_path = path.join(&file_name).path;

            entries.push(FileEntry {
                name: file_name,
                path: entry_vfs_path,
                kind,
                size,
                modified_at,
                permissions,
                mime_type,
                is_hidden,
                symlink_target,
            });
        }

        // Sort: directories first, then alphabetically
        entries.sort_by(|a, b| match (a.kind, b.kind) {
            (FileKind::Directory, FileKind::File) => std::cmp::Ordering::Less,
            (FileKind::File, FileKind::Directory) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        });

        Ok(entries)
    }

    async fn stat(&self, path: &VfsPath) -> Result<FileMetadata, VfsError> {
        let safe = self.resolve_safe(path)?;
        let abs_path = safe.absolute();

        let symlink_meta = fs::symlink_metadata(abs_path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                VfsError::NotFound(path.path.clone())
            } else {
                VfsError::IoError(e.to_string())
            }
        })?;

        let is_symlink = symlink_meta.file_type().is_symlink();
        let metadata = fs::metadata(abs_path).await.unwrap_or(symlink_meta);

        let kind = if is_symlink {
            FileKind::Symlink
        } else if metadata.is_dir() {
            FileKind::Directory
        } else {
            FileKind::File
        };

        let file_name = path
            .file_name()
            .unwrap_or_else(|| self.connection_id.as_str())
            .to_string();
        let is_hidden = file_name.starts_with('.');
        let size = metadata.len();
        let mtime = metadata.modified().ok();
        let modified_at = mtime.map(|t| DateTime::<Utc>::from(t));
        let created_at = metadata.created().ok().map(|t| DateTime::<Utc>::from(t));
        let is_readonly = metadata.permissions().readonly();
        let etag = Self::calculate_etag(size, mtime);

        #[cfg(unix)]
        let permissions = {
            use std::os::unix::fs::MetadataExt;
            Some(Self::format_permissions(metadata.mode()))
        };
        #[cfg(not(unix))]
        let permissions = None;

        let mime_type = if kind == FileKind::File {
            mime_guess::from_path(abs_path)
                .first_raw()
                .map(|s| s.to_string())
        } else {
            None
        };

        let symlink_target = if is_symlink {
            fs::read_link(abs_path)
                .await
                .ok()
                .map(|p| p.to_string_lossy().to_string())
        } else {
            None
        };

        Ok(FileMetadata {
            name: file_name,
            path: path.path.clone(),
            kind,
            size,
            modified_at,
            created_at,
            permissions,
            mime_type,
            etag,
            is_readonly,
            is_hidden,
            symlink_target,
        })
    }

    async fn read_stream(&self, path: &VfsPath) -> Result<AsyncReadBox, VfsError> {
        let safe = self.resolve_safe(path)?;
        let abs_path = safe.absolute();

        let file = File::open(abs_path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                VfsError::NotFound(path.path.clone())
            } else if e.kind() == std::io::ErrorKind::PermissionDenied {
                VfsError::PermissionDenied(path.path.clone())
            } else {
                VfsError::IoError(e.to_string())
            }
        })?;

        Ok(Box::new(file))
    }

    async fn write_stream(&self, path: &VfsPath, mut input: AsyncReadBox) -> Result<(), VfsError> {
        let safe = self.resolve_safe(path)?;
        let abs_path = safe.absolute();

        // Ensure parent directory exists
        if let Some(parent) = abs_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                VfsError::IoError(format!("Failed to create parent directory: {}", e))
            })?;
        }

        // Atomic write: write to unique .tmp file in the same directory, then rename
        let parent_dir = abs_path.parent().unwrap_or_else(|| Path::new("."));
        let temp_file_path = parent_dir.join(format!(
            ".tmp_{}_{}",
            uuid::Uuid::new_v4(),
            abs_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
        ));

        let mut temp_file = File::create(&temp_file_path).await.map_err(|e| {
            VfsError::IoError(format!("Failed to create temporary file: {}", e))
        })?;

        // Stream data from input to temp file
        let mut buffer = [0u8; 64 * 1024]; // 64KB buffer
        loop {
            use tokio::io::AsyncReadExt;
            let n = input.read(&mut buffer).await.map_err(|e| {
                VfsError::IoError(format!("Failed to read stream input: {}", e))
            })?;
            if n == 0 {
                break;
            }
            temp_file.write_all(&buffer[..n]).await.map_err(|e| {
                VfsError::IoError(format!("Failed to write to temporary file: {}", e))
            })?;
        }

        temp_file.flush().await.map_err(|e| {
            VfsError::IoError(format!("Failed to flush temporary file: {}", e))
        })?;

        temp_file.sync_all().await.map_err(|e| {
            VfsError::IoError(format!("Failed to sync temporary file to disk: {}", e))
        })?;

        drop(temp_file);

        // Atomic rename
        fs::rename(&temp_file_path, abs_path).await.map_err(|e| {
            // Clean up temp file on failure
            let _ = std::fs::remove_file(&temp_file_path);
            VfsError::IoError(format!("Failed to atomically rename file: {}", e))
        })?;

        Ok(())
    }

    async fn create_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        let safe = self.resolve_safe(path)?;
        let abs_path = safe.absolute();

        if abs_path.exists() {
            return Err(VfsError::AlreadyExists(path.path.clone()));
        }

        if let Some(parent) = abs_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                VfsError::IoError(format!("Failed to create parent directory: {}", e))
            })?;
        }

        File::create(abs_path).await.map_err(|e| {
            VfsError::IoError(format!("Failed to create file {}: {}", path.path, e))
        })?;

        Ok(())
    }

    async fn create_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        let safe = self.resolve_safe(path)?;
        let abs_path = safe.absolute();

        if abs_path.exists() {
            return Err(VfsError::AlreadyExists(path.path.clone()));
        }

        fs::create_dir_all(abs_path).await.map_err(|e| {
            VfsError::IoError(format!("Failed to create directory {}: {}", path.path, e))
        })?;

        Ok(())
    }

    async fn delete(&self, path: &VfsPath) -> Result<(), VfsError> {
        let safe = self.resolve_safe(path)?;
        let abs_path = safe.absolute();

        if !abs_path.exists() {
            return Err(VfsError::NotFound(path.path.clone()));
        }

        if abs_path.is_dir() {
            fs::remove_dir_all(abs_path).await.map_err(|e| {
                VfsError::IoError(format!("Failed to remove directory {}: {}", path.path, e))
            })?;
        } else {
            fs::remove_file(abs_path).await.map_err(|e| {
                VfsError::IoError(format!("Failed to remove file {}: {}", path.path, e))
            })?;
        }

        Ok(())
    }

    async fn rename(&self, from: &VfsPath, to: &VfsPath) -> Result<(), VfsError> {
        let safe_from = self.resolve_safe(from)?;
        let safe_to = self.resolve_safe(to)?;

        if !safe_from.absolute().exists() {
            return Err(VfsError::NotFound(from.path.clone()));
        }

        if let Some(parent) = safe_to.absolute().parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                VfsError::IoError(format!("Failed to create parent directory: {}", e))
            })?;
        }

        fs::rename(safe_from.absolute(), safe_to.absolute())
            .await
            .map_err(|e| VfsError::IoError(format!("Failed to rename: {}", e)))?;

        Ok(())
    }

    async fn copy(&self, from: &VfsPath, to: &VfsPath) -> Result<(), VfsError> {
        let safe_from = self.resolve_safe(from)?;
        let safe_to = self.resolve_safe(to)?;

        let from_abs = safe_from.absolute();
        let to_abs = safe_to.absolute();

        if !from_abs.exists() {
            return Err(VfsError::NotFound(from.path.clone()));
        }

        if from_abs.is_file() {
            if let Some(parent) = to_abs.parent() {
                fs::create_dir_all(parent).await.map_err(|e| {
                    VfsError::IoError(format!("Failed to create parent directory: {}", e))
                })?;
            }
            fs::copy(from_abs, to_abs).await.map_err(|e| {
                VfsError::IoError(format!("Failed to copy file: {}", e))
            })?;
        } else if from_abs.is_dir() {
            // Recursive directory copy
            copy_dir_all(from_abs, to_abs).await?;
        }

        Ok(())
    }
}

async fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), VfsError> {
    fs::create_dir_all(dst).await.map_err(|e| {
        VfsError::IoError(format!("Failed to create directory {:?}: {}", dst, e))
    })?;

    let mut entries = fs::read_dir(src).await.map_err(|e| {
        VfsError::IoError(format!("Failed to read source directory {:?}: {}", src, e))
    })?;

    while let Some(entry) = entries.next_entry().await.map_err(|e| {
        VfsError::IoError(format!("Failed to iterate directory entry: {}", e))
    })? {
        let entry_path = entry.path();
        let file_name = entry.file_name();
        let dest_path = dst.join(file_name);

        if entry_path.is_dir() {
            Box::pin(copy_dir_all(&entry_path, &dest_path)).await?;
        } else {
            fs::copy(&entry_path, &dest_path).await.map_err(|e| {
                VfsError::IoError(format!("Failed to copy file {:?}: {}", entry_path, e))
            })?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio::io::AsyncReadExt;

    #[tokio::test]
    async fn test_local_vfs_crud_operations() {
        let temp = tempdir().unwrap();
        let vfs = LocalFileSystem::new("local_test", temp.path(), false);

        // 1. Create directory
        let dir_path = VfsPath::new("local_test", "/docs");
        vfs.create_dir(&dir_path).await.unwrap();

        // 2. Create and write file atomically via stream
        let file_path = VfsPath::new("local_test", "/docs/hello.txt");
        let content = b"Hello, WebFileManager VFS!".to_vec();
        let cursor = std::io::Cursor::new(content.clone());
        vfs.write_stream(&file_path, Box::new(cursor)).await.unwrap();

        // 3. Stat file
        let meta = vfs.stat(&file_path).await.unwrap();
        assert_eq!(meta.name, "hello.txt");
        assert_eq!(meta.size, content.len() as u64);
        assert_eq!(meta.kind, FileKind::File);
        assert!(!meta.etag.is_empty());

        // 4. Read file stream
        let mut reader = vfs.read_stream(&file_path).await.unwrap();
        let mut read_buf = Vec::new();
        reader.read_to_end(&mut read_buf).await.unwrap();
        assert_eq!(read_buf, content);

        // 5. List directory
        let list = vfs.list(&dir_path).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "hello.txt");

        // 6. Rename
        let renamed_path = VfsPath::new("local_test", "/docs/greeting.txt");
        vfs.rename(&file_path, &renamed_path).await.unwrap();
        assert!(vfs.stat(&renamed_path).await.is_ok());
        assert!(vfs.stat(&file_path).await.is_err());

        // 7. Copy
        let copied_path = VfsPath::new("local_test", "/docs/greeting_copy.txt");
        vfs.copy(&renamed_path, &copied_path).await.unwrap();
        assert!(vfs.stat(&copied_path).await.is_ok());

        // 8. Delete
        vfs.delete(&copied_path).await.unwrap();
        assert!(vfs.stat(&copied_path).await.is_err());
    }

    #[tokio::test]
    async fn test_local_vfs_traversal_rejection() {
        let temp = tempdir().unwrap();
        let sandbox_dir = temp.path().join("sandbox");
        let outside_dir = temp.path().join("outside");
        std::fs::create_dir_all(&sandbox_dir).unwrap();
        std::fs::create_dir_all(&outside_dir).unwrap();

        let outside_file = outside_dir.join("hosts");
        std::fs::write(&outside_file, "127.0.0.1 localhost").unwrap();

        let vfs = LocalFileSystem::new("local_test", &sandbox_dir, false);

        // 1. Path traversal via VfsPath cannot access host /etc/passwd or outside files
        let attack_path = VfsPath::new("local_test", "../../../etc/passwd");
        let res = vfs.stat(&attack_path).await;
        // It must NOT read host's /etc/passwd; it stays inside sandbox root where etc/passwd does not exist
        assert!(res.is_err());
        assert!(matches!(res.unwrap_err(), VfsError::NotFound(_)));

        // 2. Malicious symlink pointing outside sandbox must return PermissionDenied
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let escape_link = sandbox_dir.join("malicious_symlink");
            symlink(&outside_dir, &escape_link).unwrap();

            let symlink_attack_path = VfsPath::new("local_test", "/malicious_symlink/hosts");
            let res2 = vfs.stat(&symlink_attack_path).await;
            assert!(res2.is_err());
            assert!(matches!(res2.unwrap_err(), VfsError::PermissionDenied(_)));
        }
    }
}
