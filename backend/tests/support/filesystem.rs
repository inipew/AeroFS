use async_trait::async_trait;
use backend::{
    domain::{Capabilities, ConnectionId, FileEntry, FileKind, FileMetadata, VfsPath},
    errors::VfsError,
    ports::filesystem::FileSystemResolver,
    vfs::{AsyncReadBox, FileSystem},
};
use bytes::Bytes;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use tokio::io::AsyncReadExt;
use tokio_util::io::StreamReader;

#[derive(Default)]
pub struct MemoryFileSystem {
    files: RwLock<HashMap<String, Vec<u8>>>,
    directories: RwLock<HashSet<String>>,
    permissions: RwLock<HashMap<String, String>>,
    capabilities: RwLock<Capabilities>,
    permission_failure: RwLock<Option<String>>,
}

impl MemoryFileSystem {
    pub fn new(capabilities: Capabilities) -> Self {
        Self {
            capabilities: RwLock::new(capabilities),
            ..Self::default()
        }
    }

    pub fn insert_file(&self, path: &str, content: impl Into<Vec<u8>>) {
        self.files
            .write()
            .expect("memory fs files lock")
            .insert(normalize(path), content.into());
    }

    pub fn insert_dir(&self, path: &str) {
        self.directories
            .write()
            .expect("memory fs directories lock")
            .insert(normalize(path));
    }

    pub fn bytes(&self, path: &str) -> Option<Vec<u8>> {
        self.files
            .read()
            .expect("memory fs files lock")
            .get(&normalize(path))
            .cloned()
    }

    pub fn permission(&self, path: &str) -> Option<String> {
        self.permissions
            .read()
            .expect("memory fs permissions lock")
            .get(&normalize(path))
            .cloned()
    }

    pub fn fail_permissions(&self, message: impl Into<String>) {
        *self
            .permission_failure
            .write()
            .expect("memory fs permission failure lock") = Some(message.into());
    }

    pub fn clear_permission_failure(&self) {
        *self
            .permission_failure
            .write()
            .expect("memory fs permission failure lock") = None;
    }
}

fn normalize(path: &str) -> String {
    if path.is_empty() || path == "/" {
        "/".to_string()
    } else if path.starts_with('/') {
        path.trim_end_matches('/').to_string()
    } else {
        format!("/{}", path.trim_end_matches('/'))
    }
}

fn parent_of(path: &str) -> String {
    let path = normalize(path);
    if path == "/" {
        return "/".to_string();
    }
    path.rsplit_once('/')
        .map(|(parent, _)| if parent.is_empty() { "/" } else { parent })
        .unwrap_or("/")
        .to_string()
}

fn name_of(path: &str) -> String {
    normalize(path)
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_string()
}

#[async_trait]
impl FileSystem for MemoryFileSystem {
    fn capabilities(&self) -> Capabilities {
        self.capabilities
            .read()
            .expect("memory fs capabilities lock")
            .clone()
    }

    async fn list_stream(
        &self,
        path: &VfsPath,
    ) -> Result<backend::vfs::traits::FileStreamBox, VfsError> {
        let parent = normalize(&path.path);
        let files = self.files.read().expect("memory fs files lock");
        let dirs = self.directories.read().expect("memory fs directories lock");
        let permissions = self.permissions.read().expect("memory fs permissions lock");
        let mut entries = Vec::new();

        for (child, content) in files.iter() {
            if parent_of(child) == parent {
                entries.push(FileEntry {
                    name: name_of(child),
                    path: child.clone(),
                    kind: FileKind::File,
                    size: Some(content.len() as u64),
                    modified_at: None,
                    permissions: permissions.get(child).cloned(),
                    mime_type: None,
                    is_hidden: name_of(child).starts_with('.'),
                    symlink_target: None,
                });
            }
        }
        for child in dirs.iter() {
            if child != "/" && parent_of(child) == parent {
                entries.push(FileEntry {
                    name: name_of(child),
                    path: child.clone(),
                    kind: FileKind::Directory,
                    size: None,
                    modified_at: None,
                    permissions: permissions.get(child).cloned(),
                    mime_type: None,
                    is_hidden: name_of(child).starts_with('.'),
                    symlink_target: None,
                });
            }
        }
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(Box::pin(futures::stream::iter(entries.into_iter().map(Ok))))
    }

    async fn stat(&self, path: &VfsPath) -> Result<FileMetadata, VfsError> {
        let path = normalize(&path.path);
        let permissions = self.permissions.read().expect("memory fs permissions lock");
        if let Some(content) = self.files.read().expect("memory fs files lock").get(&path) {
            return Ok(FileMetadata {
                name: name_of(&path),
                path: path.clone(),
                kind: FileKind::File,
                size: content.len() as u64,
                modified_at: None,
                created_at: None,
                permissions: permissions.get(&path).cloned(),
                mime_type: None,
                etag: format!("memory:{}:{}", path, content.len()),
                is_readonly: false,
                is_hidden: name_of(&path).starts_with('.'),
                symlink_target: None,
            });
        }
        if path == "/"
            || self
                .directories
                .read()
                .expect("memory fs directories lock")
                .contains(&path)
        {
            return Ok(FileMetadata {
                name: name_of(&path),
                path: path.clone(),
                kind: FileKind::Directory,
                size: 0,
                modified_at: None,
                created_at: None,
                permissions: permissions.get(&path).cloned(),
                mime_type: None,
                etag: format!("memory-dir:{path}"),
                is_readonly: false,
                is_hidden: false,
                symlink_target: None,
            });
        }
        Err(VfsError::NotFound(path))
    }

    async fn read_stream(&self, path: &VfsPath) -> Result<AsyncReadBox, VfsError> {
        let content = self
            .bytes(&path.path)
            .ok_or_else(|| VfsError::NotFound(path.path.clone()))?;
        let stream = futures::stream::iter(vec![Ok::<Bytes, std::io::Error>(Bytes::from(content))]);
        Ok(Box::new(StreamReader::new(stream)))
    }

    async fn read_range(
        &self,
        path: &VfsPath,
        offset: u64,
        length: u64,
    ) -> Result<AsyncReadBox, VfsError> {
        let content = self
            .bytes(&path.path)
            .ok_or_else(|| VfsError::NotFound(path.path.clone()))?;
        let start = (offset as usize).min(content.len());
        let end = start.saturating_add(length as usize).min(content.len());
        let stream = futures::stream::iter(vec![Ok::<Bytes, std::io::Error>(
            Bytes::copy_from_slice(&content[start..end]),
        )]);
        Ok(Box::new(StreamReader::new(stream)))
    }

    async fn write_stream(&self, path: &VfsPath, mut input: AsyncReadBox) -> Result<(), VfsError> {
        let mut content = Vec::new();
        input
            .read_to_end(&mut content)
            .await
            .map_err(|error| VfsError::IoError(error.to_string()))?;
        self.insert_file(&path.path, content);
        Ok(())
    }

    async fn create_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.insert_file(&path.path, Vec::<u8>::new());
        Ok(())
    }

    async fn create_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.insert_dir(&path.path);
        Ok(())
    }

    async fn delete(&self, path: &VfsPath) -> Result<(), VfsError> {
        let path = normalize(&path.path);
        self.files.write().expect("memory fs files lock").remove(&path);
        self.directories
            .write()
            .expect("memory fs directories lock")
            .remove(&path);
        Ok(())
    }

    async fn rename(&self, from: &VfsPath, to: &VfsPath) -> Result<(), VfsError> {
        let from = normalize(&from.path);
        let to = normalize(&to.path);
        if let Some(content) = self.files.write().expect("memory fs files lock").remove(&from) {
            self.files
                .write()
                .expect("memory fs files lock")
                .insert(to, content);
            return Ok(());
        }
        Err(VfsError::NotFound(from))
    }

    async fn copy(&self, from: &VfsPath, to: &VfsPath) -> Result<(), VfsError> {
        let content = self
            .bytes(&from.path)
            .ok_or_else(|| VfsError::NotFound(from.path.clone()))?;
        self.insert_file(&to.path, content);
        Ok(())
    }

    async fn set_permissions(&self, path: &VfsPath, permissions: &str) -> Result<(), VfsError> {
        if let Some(message) = self
            .permission_failure
            .read()
            .expect("memory fs permission failure lock")
            .clone()
        {
            return Err(VfsError::IoError(message));
        }
        self.permissions
            .write()
            .expect("memory fs permissions lock")
            .insert(normalize(&path.path), permissions.to_string());
        Ok(())
    }
}

#[derive(Clone)]
pub struct SwappableFileSystemResolver {
    provider: Arc<RwLock<Arc<dyn FileSystem>>>,
}

impl SwappableFileSystemResolver {
    pub fn new(provider: Arc<dyn FileSystem>) -> Self {
        Self {
            provider: Arc::new(RwLock::new(provider)),
        }
    }

    pub fn replace(&self, provider: Arc<dyn FileSystem>) {
        *self.provider.write().expect("resolver provider lock") = provider;
    }
}

#[async_trait]
impl FileSystemResolver for SwappableFileSystemResolver {
    async fn resolve(&self, _connection: &ConnectionId) -> Result<Arc<dyn FileSystem>, VfsError> {
        Ok(self.provider.read().expect("resolver provider lock").clone())
    }
}
