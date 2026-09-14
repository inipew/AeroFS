use async_trait::async_trait;
use backend::domain::{Capabilities, FileEntry, FileKind, FileMetadata, VfsPath};
use backend::errors::VfsError;
use backend::filesystem::archive::ArchiveOverwriteMode;
use backend::filesystem::archive_stream::{extract_targz_streaming, extract_zip_streaming};
use backend::vfs::traits::FileStreamBox;
use backend::vfs::{AsyncReadBox, FileSystem};
use bytes::Bytes;
use flate2::{write::GzEncoder, Compression};
use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Write};
use std::sync::{Arc, RwLock};
use tokio::io::AsyncReadExt;
use tokio_util::io::StreamReader;
use zip::write::SimpleFileOptions;

#[derive(Default)]
struct MemoryFs {
    files: RwLock<HashMap<String, Vec<u8>>>,
    directories: RwLock<HashSet<String>>,
}

impl MemoryFs {
    fn insert_file(&self, path: &str, content: Vec<u8>) {
        self.files.write().unwrap().insert(path.to_string(), content);
    }

    fn bytes(&self, path: &str) -> Option<Vec<u8>> {
        self.files.read().unwrap().get(path).cloned()
    }
}

#[async_trait]
impl FileSystem for MemoryFs {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            read: true,
            write: true,
            ..Default::default()
        }
    }

    async fn list_stream(&self, _path: &VfsPath) -> Result<FileStreamBox, VfsError> {
        Ok(Box::pin(futures::stream::empty::<Result<FileEntry, VfsError>>()))
    }

    async fn stat(&self, path: &VfsPath) -> Result<FileMetadata, VfsError> {
        if let Some(content) = self.files.read().unwrap().get(&path.path) {
            return Ok(FileMetadata {
                name: path.path.rsplit('/').next().unwrap_or_default().to_string(),
                path: path.path.clone(),
                kind: FileKind::File,
                size: content.len() as u64,
                modified_at: None,
                created_at: None,
                permissions: None,
                mime_type: None,
                etag: String::new(),
                is_readonly: false,
                is_hidden: false,
                symlink_target: None,
            });
        }
        if self.directories.read().unwrap().contains(&path.path) {
            return Ok(FileMetadata {
                name: path.path.rsplit('/').next().unwrap_or_default().to_string(),
                path: path.path.clone(),
                kind: FileKind::Directory,
                size: 0,
                modified_at: None,
                created_at: None,
                permissions: None,
                mime_type: None,
                etag: String::new(),
                is_readonly: false,
                is_hidden: false,
                symlink_target: None,
            });
        }
        Err(VfsError::NotFound(path.path.clone()))
    }

    async fn read_stream(&self, path: &VfsPath) -> Result<AsyncReadBox, VfsError> {
        let content = self
            .files
            .read()
            .unwrap()
            .get(&path.path)
            .cloned()
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
            .files
            .read()
            .unwrap()
            .get(&path.path)
            .cloned()
            .ok_or_else(|| VfsError::NotFound(path.path.clone()))?;
        let start = (offset as usize).min(content.len());
        let end = start.saturating_add(length as usize).min(content.len());
        let stream = futures::stream::iter(vec![Ok::<Bytes, std::io::Error>(Bytes::copy_from_slice(
            &content[start..end],
        ))]);
        Ok(Box::new(StreamReader::new(stream)))
    }

    async fn write_stream(&self, path: &VfsPath, mut input: AsyncReadBox) -> Result<(), VfsError> {
        let mut content = Vec::new();
        input
            .read_to_end(&mut content)
            .await
            .map_err(|error| VfsError::IoError(error.to_string()))?;
        self.files.write().unwrap().insert(path.path.clone(), content);
        Ok(())
    }

    async fn create_file(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.files
            .write()
            .unwrap()
            .entry(path.path.clone())
            .or_default();
        Ok(())
    }

    async fn create_dir(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.directories.write().unwrap().insert(path.path.clone());
        Ok(())
    }

    async fn delete(&self, path: &VfsPath) -> Result<(), VfsError> {
        self.files.write().unwrap().remove(&path.path);
        self.directories.write().unwrap().remove(&path.path);
        Ok(())
    }

    async fn rename(&self, from: &VfsPath, to: &VfsPath) -> Result<(), VfsError> {
        let content = self
            .files
            .write()
            .unwrap()
            .remove(&from.path)
            .ok_or_else(|| VfsError::NotFound(from.path.clone()))?;
        self.files.write().unwrap().insert(to.path.clone(), content);
        Ok(())
    }

    async fn copy(&self, from: &VfsPath, to: &VfsPath) -> Result<(), VfsError> {
        let content = self
            .files
            .read()
            .unwrap()
            .get(&from.path)
            .cloned()
            .ok_or_else(|| VfsError::NotFound(from.path.clone()))?;
        self.files.write().unwrap().insert(to.path.clone(), content);
        Ok(())
    }
}

fn zip_bytes(path: &str, content: &[u8]) -> Vec<u8> {
    let cursor = Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(cursor);
    writer
        .start_file(
            path,
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
        )
        .unwrap();
    writer.write_all(content).unwrap();
    writer.finish().unwrap().into_inner()
}

fn targz_bytes(path: &str, content: &[u8]) -> Vec<u8> {
    let encoder = GzEncoder::new(Vec::new(), Compression::default());
    let mut builder = tar::Builder::new(encoder);
    let mut header = tar::Header::new_gnu();
    header.set_size(content.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    builder
        .append_data(&mut header, path, content)
        .expect("append tar entry");
    let encoder = builder.into_inner().expect("finish tar builder");
    encoder.finish().expect("finish gzip")
}

#[tokio::test]
async fn zip_full_extract_streams_nested_file_to_provider() {
    let fs = Arc::new(MemoryFs::default());
    fs.insert_file("/input.zip", zip_bytes("nested/hello.txt", b"hello zip"));
    let provider: Arc<dyn FileSystem> = fs.clone();
    let archive = VfsPath::new("memory", "/input.zip").unwrap();

    let (extracted, skipped) = extract_zip_streaming(
        &provider,
        &archive,
        "/output",
        ArchiveOverwriteMode::Overwrite,
    )
    .await
    .unwrap();

    assert_eq!((extracted, skipped), (1, 0));
    assert_eq!(fs.bytes("/output/nested/hello.txt").as_deref(), Some(b"hello zip".as_slice()));
}

#[tokio::test]
async fn targz_full_extract_streams_nested_file_to_provider() {
    let fs = Arc::new(MemoryFs::default());
    fs.insert_file(
        "/input.tar.gz",
        targz_bytes("nested/hello.txt", b"hello targz"),
    );
    let provider: Arc<dyn FileSystem> = fs.clone();
    let archive = VfsPath::new("memory", "/input.tar.gz").unwrap();

    let (extracted, skipped) = extract_targz_streaming(
        &provider,
        &archive,
        "/output",
        ArchiveOverwriteMode::Overwrite,
    )
    .await
    .unwrap();

    assert_eq!((extracted, skipped), (1, 0));
    assert_eq!(
        fs.bytes("/output/nested/hello.txt").as_deref(),
        Some(b"hello targz".as_slice())
    );
}

#[tokio::test]
async fn zip_skip_mode_preserves_existing_destination_without_streaming_replacement() {
    let fs = Arc::new(MemoryFs::default());
    fs.insert_file("/input.zip", zip_bytes("hello.txt", b"replacement"));
    fs.insert_file("/output/hello.txt", b"original".to_vec());
    let provider: Arc<dyn FileSystem> = fs.clone();
    let archive = VfsPath::new("memory", "/input.zip").unwrap();

    let (extracted, skipped) = extract_zip_streaming(
        &provider,
        &archive,
        "/output",
        ArchiveOverwriteMode::Skip,
    )
    .await
    .unwrap();

    assert_eq!((extracted, skipped), (0, 1));
    assert_eq!(fs.bytes("/output/hello.txt").as_deref(), Some(b"original".as_slice()));
}
