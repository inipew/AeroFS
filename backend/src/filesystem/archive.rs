use crate::domain::{FileKind, VfsPath};
use crate::errors::{SecurityError, VfsError};
use crate::vfs::FileSystem;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{Cursor, Read};
use std::path::Path;
use std::sync::{Arc, LazyLock, RwLock};
use std::time::Instant;
use tokio::io::AsyncReadExt;
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    Zip,
    TarGz,
}

impl ArchiveFormat {
    pub fn from_path(path: &str) -> Option<Self> {
        let p = path.to_lowercase();
        if p.ends_with(".zip") {
            Some(Self::Zip)
        } else if p.ends_with(".tar.gz") || p.ends_with(".tgz") {
            Some(Self::TarGz)
        } else {
            None
        }
    }
}

/// Validate archive entry path against Zip Slip, Directory Traversal, NUL bytes, Absolute paths, and Windows Prefixes
pub fn validate_archive_entry_path(raw_name: &str) -> Result<String, SecurityError> {
    if raw_name.contains('\0') {
        return Err(SecurityError::PathTraversal("NUL byte detected in archive entry path".into()));
    }

    let path_obj = Path::new(raw_name);
    if path_obj.is_absolute() {
        return Err(SecurityError::PathTraversal(format!("Absolute path in archive rejected: {}", raw_name)));
    }

    for comp in path_obj.components() {
        match comp {
            std::path::Component::ParentDir => {
                return Err(SecurityError::PathTraversal(format!(
                    "Directory traversal ('..') in archive rejected: {}",
                    raw_name
                )));
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                return Err(SecurityError::PathTraversal(format!(
                    "Invalid path root or drive prefix in archive rejected: {}",
                    raw_name
                )));
            }
            _ => {}
        }
    }

    Ok(raw_name.trim_start_matches('/').to_string())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct VirtualArchiveEntry {
    pub name: String,
    pub path: String,
    pub kind: String, // "file" or "directory"
    pub size: u64,
    pub compressed_size: Option<u64>,
    pub modified_at: Option<String>,
}

#[derive(Debug, Clone)]
struct CachedArchiveIndex {
    _cached_at: Instant,
    mtime: Option<chrono::DateTime<chrono::Utc>>,
    size: u64,
    all_entries: Vec<VirtualArchiveEntry>,
}

static ARCHIVE_CACHE: LazyLock<RwLock<HashMap<String, CachedArchiveIndex>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

fn get_cache_key(archive_path: &VfsPath) -> String {
    format!("{}:{}", archive_path.connection_id, archive_path.path)
}

/// Helper: Stream an AsyncRead into a std::io::Write (e.g. temporary file) in 64 KiB chunks
async fn stream_async_to_sync_writer<W: std::io::Write>(
    mut reader: Box<dyn tokio::io::AsyncRead + Unpin + Send>,
    writer: &mut W,
) -> Result<u64, VfsError> {
    let mut buf = [0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let n = reader
            .read(&mut buf)
            .await
            .map_err(|e| VfsError::IoError(format!("Read error: {}", e)))?;
        if n == 0 {
            break;
        }
        writer
            .write_all(&buf[..n])
            .map_err(|e| VfsError::IoError(format!("Write error: {}", e)))?;
        total += n as u64;
    }
    writer
        .flush()
        .map_err(|e| VfsError::IoError(format!("Flush error: {}", e)))?;
    Ok(total)
}

/// Compress selected files into a ZIP archive via streaming
pub async fn compress_zip(
    provider: &Arc<dyn FileSystem>,
    connection_id: &str,
    base_dir: &str,
    relative_paths: &[String],
    target_zip_path: &VfsPath,
) -> Result<(), VfsError> {
    let temp_file = tempfile::NamedTempFile::new()
        .map_err(|e| VfsError::IoError(format!("Failed to create temp zip file: {}", e)))?;
    let temp_path = temp_file.path().to_path_buf();

    {
        let file = std::fs::File::create(&temp_path)
            .map_err(|e| VfsError::IoError(format!("Failed opening temp file: {}", e)))?;
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        for rel in relative_paths {
            let full_vfs = if base_dir == "/" {
                VfsPath::new(connection_id, format!("/{}", rel.trim_start_matches('/')))
            } else {
                VfsPath::new(
                    connection_id,
                    format!("{}/{}", base_dir.trim_end_matches('/'), rel.trim_start_matches('/')),
                )
            };

            let meta = provider.stat(&full_vfs).await?;
            if meta.kind == FileKind::File {
                let reader = provider.read_stream(&full_vfs).await?;
                zip.start_file(rel.trim_start_matches('/'), options)
                    .map_err(|e| VfsError::IoError(format!("Zip entry error: {}", e)))?;

                // Stream file in 64 KiB chunks directly into zip compressor
                stream_async_to_sync_writer(reader, &mut zip).await?;
            }
        }
        zip.finish()
            .map_err(|e| VfsError::IoError(format!("Zip finalize error: {}", e)))?;
    }

    // Stream finished archive file to target VFS path
    let async_file = tokio::fs::File::open(&temp_path)
        .await
        .map_err(|e| VfsError::IoError(format!("Failed opening output zip file: {}", e)))?;
    provider
        .write_stream(target_zip_path, Box::new(async_file))
        .await?;

    let _ = tokio::fs::remove_file(&temp_path).await;
    Ok(())
}

/// Extract a ZIP archive with streaming decompression and strict Zip Slip protection
pub async fn extract_zip(
    provider: &Arc<dyn FileSystem>,
    archive_path: &VfsPath,
    target_dir: &str,
) -> Result<usize, VfsError> {
    let temp_file = tempfile::NamedTempFile::new()
        .map_err(|e| VfsError::IoError(format!("Failed to create temp extract file: {}", e)))?;
    let temp_path = temp_file.path().to_path_buf();

    // 1. Stream archive to temp file with 64 KiB chunks
    {
        let mut file = std::fs::File::create(&temp_path)
            .map_err(|e| VfsError::IoError(format!("Failed opening temp file: {}", e)))?;
        let reader = provider.read_stream(archive_path).await?;
        stream_async_to_sync_writer(reader, &mut file).await?;
    }

    // 2. Extract entries to temporary directory in blocking task (fast, bounded memory)
    let temp_dir_obj = tempfile::tempdir()
        .map_err(|e| VfsError::IoError(format!("Failed creating temp dir: {}", e)))?;
    let staging_root = temp_dir_obj.path().to_path_buf();

    let staging_clone = staging_root.clone();
    let temp_path_clone = temp_path.clone();

    #[derive(Clone)]
    struct ExtractedItem {
        rel_path: String,
        is_dir: bool,
    }

    let items = tokio::task::spawn_blocking(move || -> Result<Vec<ExtractedItem>, VfsError> {
        let file = std::fs::File::open(&temp_path_clone)
            .map_err(|e| VfsError::IoError(format!("Failed opening downloaded archive: {}", e)))?;
        let mut zip = ZipArchive::new(file)
            .map_err(|e| VfsError::IoError(format!("Invalid zip archive: {}", e)))?;

        const MAX_ENTRIES: usize = 50_000;
        const MAX_UNCOMPRESSED_BYTES: u64 = 50 * 1024 * 1024 * 1024; // 50 GB limit

        if zip.len() > MAX_ENTRIES {
            return Err(SecurityError::PathTraversal(format!(
                "Zip archive rejected: contains too many entries ({} > {})",
                zip.len(),
                MAX_ENTRIES
            ))
            .into());
        }

        let mut total_uncompressed_bytes = 0u64;
        let mut list = Vec::new();

        for i in 0..zip.len() {
            let mut zip_entry = zip
                .by_index(i)
                .map_err(|e| VfsError::IoError(format!("Failed reading zip index {}: {}", i, e)))?;

            let raw_name = zip_entry.name().to_string();
            let safe_name = validate_archive_entry_path(&raw_name)?;

            let is_dir = zip_entry.is_dir();
            let dest_on_disk = staging_clone.join(&safe_name);

            if is_dir {
                std::fs::create_dir_all(&dest_on_disk)
                    .map_err(|e| VfsError::IoError(format!("Failed creating dir: {}", e)))?;
            } else {
                if let Some(parent) = dest_on_disk.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| VfsError::IoError(format!("Failed creating parent dir: {}", e)))?;
                }

                total_uncompressed_bytes += zip_entry.size();
                if total_uncompressed_bytes > MAX_UNCOMPRESSED_BYTES {
                    return Err(SecurityError::PathTraversal(
                        "Zip bomb detected: exceeded max uncompressed size limit".into(),
                    )
                    .into());
                }

                let mut out_file = std::fs::File::create(&dest_on_disk)
                    .map_err(|e| VfsError::IoError(format!("Failed creating output file: {}", e)))?;
                std::io::copy(&mut zip_entry, &mut out_file)
                    .map_err(|e| VfsError::IoError(format!("Failed decompressing {}: {}", safe_name, e)))?;
            }

            list.push(ExtractedItem {
                rel_path: safe_name,
                is_dir,
            });
        }

        Ok(list)
    })
    .await
    .map_err(|e| VfsError::IoError(format!("Blocking extract panicked: {}", e)))??;

    // 3. Stream extracted items to provider VFS
    let mut extracted_count = 0;
    for item in items {
        let full_dest_path = if target_dir == "/" {
            format!("/{}", item.rel_path.trim_start_matches('/'))
        } else {
            format!(
                "{}/{}",
                target_dir.trim_end_matches('/'),
                item.rel_path.trim_start_matches('/')
            )
        };
        let dest_vfs = VfsPath::new(&archive_path.connection_id, &full_dest_path);

        if item.is_dir {
            let _ = provider.create_dir(&dest_vfs).await;
        } else {
            if let Some(parent) = dest_vfs.parent() {
                let _ = provider.create_dir(&parent).await;
            }
            let disk_file = staging_root.join(&item.rel_path);
            let async_reader = tokio::fs::File::open(&disk_file)
                .await
                .map_err(|e| VfsError::IoError(format!("Failed reading staged file: {}", e)))?;
            provider.write_stream(&dest_vfs, Box::new(async_reader)).await?;
            extracted_count += 1;
        }
    }

    let _ = tokio::fs::remove_file(&temp_path).await;
    Ok(extracted_count)
}

/// Compress selected files into a TAR.GZ archive via streaming
pub async fn compress_targz(
    provider: &Arc<dyn FileSystem>,
    connection_id: &str,
    base_dir: &str,
    relative_paths: &[String],
    target_targz_path: &VfsPath,
) -> Result<(), VfsError> {
    let temp_file = tempfile::NamedTempFile::new()
        .map_err(|e| VfsError::IoError(format!("Failed to create temp targz file: {}", e)))?;
    let temp_path = temp_file.path().to_path_buf();

    {
        let file = std::fs::File::create(&temp_path)
            .map_err(|e| VfsError::IoError(format!("Failed opening temp file: {}", e)))?;
        let enc = GzEncoder::new(file, Compression::default());
        let mut tar_builder = tar::Builder::new(enc);

        for rel in relative_paths {
            let full_vfs = if base_dir == "/" {
                VfsPath::new(connection_id, format!("/{}", rel.trim_start_matches('/')))
            } else {
                VfsPath::new(
                    connection_id,
                    format!("{}/{}", base_dir.trim_end_matches('/'), rel.trim_start_matches('/')),
                )
            };

            let meta = provider.stat(&full_vfs).await?;
            if meta.kind == FileKind::File {
                let mut reader = provider.read_stream(&full_vfs).await?;
                let mut content = Vec::new();
                reader.read_to_end(&mut content).await.map_err(|e| {
                    VfsError::IoError(format!("Failed to read file {}: {}", full_vfs.path, e))
                })?;

                let mut header = tar::Header::new_gnu();
                header.set_size(content.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();

                tar_builder
                    .append_data(&mut header, rel.trim_start_matches('/'), Cursor::new(content))
                    .map_err(|e| VfsError::IoError(format!("Tar append error: {}", e)))?;
            }
        }

        tar_builder
            .into_inner()
            .map_err(|e| VfsError::IoError(format!("Tar finish error: {}", e)))?
            .finish()
            .map_err(|e| VfsError::IoError(format!("Gzip finish error: {}", e)))?;
    }

    // Stream finished archive file to target VFS path
    let async_file = tokio::fs::File::open(&temp_path)
        .await
        .map_err(|e| VfsError::IoError(format!("Failed opening output targz file: {}", e)))?;
    provider
        .write_stream(target_targz_path, Box::new(async_file))
        .await?;

    let _ = tokio::fs::remove_file(&temp_path).await;
    Ok(())
}

/// Extract a TAR.GZ archive with streaming decompression
pub async fn extract_targz(
    provider: &Arc<dyn FileSystem>,
    archive_path: &VfsPath,
    target_dir: &str,
) -> Result<usize, VfsError> {
    let temp_file = tempfile::NamedTempFile::new()
        .map_err(|e| VfsError::IoError(format!("Failed to create temp extract file: {}", e)))?;
    let temp_path = temp_file.path().to_path_buf();

    // 1. Stream archive to temp file with 64 KiB chunks
    {
        let mut file = std::fs::File::create(&temp_path)
            .map_err(|e| VfsError::IoError(format!("Failed opening temp file: {}", e)))?;
        let reader = provider.read_stream(archive_path).await?;
        stream_async_to_sync_writer(reader, &mut file).await?;
    }

    // 2. Extract entries to temporary directory in blocking task
    let temp_dir_obj = tempfile::tempdir()
        .map_err(|e| VfsError::IoError(format!("Failed creating temp dir: {}", e)))?;
    let staging_root = temp_dir_obj.path().to_path_buf();

    let staging_clone = staging_root.clone();
    let temp_path_clone = temp_path.clone();

    #[derive(Clone)]
    struct ExtractedItem {
        rel_path: String,
        is_dir: bool,
    }

    let items = tokio::task::spawn_blocking(move || -> Result<Vec<ExtractedItem>, VfsError> {
        let file = std::fs::File::open(&temp_path_clone)
            .map_err(|e| VfsError::IoError(format!("Failed opening archive: {}", e)))?;
        let gz_decoder = GzDecoder::new(file);
        let mut archive = tar::Archive::new(gz_decoder);

        let entries = archive
            .entries()
            .map_err(|e| VfsError::IoError(format!("Invalid tar archive: {}", e)))?;

        const MAX_ENTRIES: usize = 50_000;
        const MAX_UNCOMPRESSED_BYTES: u64 = 50 * 1024 * 1024 * 1024; // 50 GB limit
        let mut total_uncompressed_bytes: u64 = 0;
        let mut entry_count: usize = 0;

        let mut list = Vec::new();
        for entry_res in entries {
            entry_count += 1;
            if entry_count > MAX_ENTRIES {
                return Err(SecurityError::PathTraversal(format!(
                    "Tar archive rejected: contains too many entries (> {})",
                    MAX_ENTRIES
                ))
                .into());
            }

            let mut entry = entry_res
                .map_err(|e| VfsError::IoError(format!("Tar entry read error: {}", e)))?;

            let entry_type = entry.header().entry_type();
            // Block special devices, fifos, block devices, symlinks pointing outside
            if entry_type.is_symlink() || entry_type.is_hard_link() || entry_type.is_fifo() || entry_type.is_character_special() || entry_type.is_block_special() {
                continue; // Skip dangerous special nodes
            }

            let path_buf = entry
                .path()
                .map_err(|e| VfsError::IoError(format!("Invalid path in tar: {}", e)))?
                .to_path_buf();

            let raw_name = path_buf.to_string_lossy().to_string();
            let safe_name = validate_archive_entry_path(&raw_name)?;

            let is_dir = entry_type.is_dir();
            let dest_on_disk = staging_clone.join(&safe_name);

            if is_dir {
                std::fs::create_dir_all(&dest_on_disk)
                    .map_err(|e| VfsError::IoError(format!("Failed creating dir: {}", e)))?;
            } else {
                if let Some(parent) = dest_on_disk.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| VfsError::IoError(format!("Failed creating parent dir: {}", e)))?;
                }

                let mut out_file = std::fs::File::create(&dest_on_disk)
                    .map_err(|e| VfsError::IoError(format!("Failed creating output file: {}", e)))?;
                let copied = std::io::copy(&mut entry, &mut out_file)
                    .map_err(|e| VfsError::IoError(format!("Failed reading tar content: {}", e)))?;
                
                total_uncompressed_bytes += copied;
                if total_uncompressed_bytes > MAX_UNCOMPRESSED_BYTES {
                    return Err(SecurityError::PathTraversal(
                        "Tar archive decompression size limit exceeded (50 GB)".to_string(),
                    )
                    .into());
                }
            }

            list.push(ExtractedItem {
                rel_path: safe_name,
                is_dir,
            });
        }

        Ok(list)
    })
    .await
    .map_err(|e| VfsError::IoError(format!("Blocking tar extract panicked: {}", e)))??;

    // 3. Stream extracted items to provider VFS
    let mut extracted_count = 0;
    for item in items {
        let full_dest_path = if target_dir == "/" {
            format!("/{}", item.rel_path.trim_start_matches('/'))
        } else {
            format!(
                "{}/{}",
                target_dir.trim_end_matches('/'),
                item.rel_path.trim_start_matches('/')
            )
        };

        let dest_vfs = VfsPath::new(&archive_path.connection_id, &full_dest_path);

        if item.is_dir {
            let _ = provider.create_dir(&dest_vfs).await;
        } else {
            if let Some(parent) = dest_vfs.parent() {
                let _ = provider.create_dir(&parent).await;
            }

            let disk_file = staging_root.join(&item.rel_path);
            let async_reader = tokio::fs::File::open(&disk_file)
                .await
                .map_err(|e| VfsError::IoError(format!("Failed reading staged file: {}", e)))?;
            provider.write_stream(&dest_vfs, Box::new(async_reader)).await?;
            extracted_count += 1;
        }
    }

    let _ = tokio::fs::remove_file(&temp_path).await;
    Ok(extracted_count)
}

/// Helper function to filter all indexed archive entries for a specific subpath in O(1) memory
fn filter_entries_by_subpath(
    all_entries: &[VirtualArchiveEntry],
    subpath: &str,
) -> Vec<VirtualArchiveEntry> {
    let clean_subpath = subpath.trim_matches('/');
    let subpath_prefix = if clean_subpath.is_empty() {
        String::new()
    } else {
        format!("{}/", clean_subpath)
    };

    let mut directories = BTreeMap::<String, VirtualArchiveEntry>::new();
    let mut files = BTreeMap::<String, VirtualArchiveEntry>::new();

    for entry in all_entries {
        let clean_path = entry.path.trim_matches('/');
        if clean_path.is_empty() {
            continue;
        }

        // Check if inside requested subpath
        if !subpath_prefix.is_empty() && !clean_path.starts_with(&subpath_prefix) {
            continue;
        }

        let remainder = if subpath_prefix.is_empty() {
            clean_path
        } else {
            &clean_path[subpath_prefix.len()..]
        };

        if remainder.is_empty() {
            continue;
        }

        let parts: Vec<&str> = remainder.split('/').collect();
        if parts.len() == 1 && entry.kind == "file" {
            // Direct file in this subfolder
            files.insert(
                parts[0].to_string(),
                VirtualArchiveEntry {
                    name: parts[0].to_string(),
                    path: if clean_subpath.is_empty() {
                        parts[0].to_string()
                    } else {
                        format!("{}/{}", clean_subpath, parts[0])
                    },
                    kind: "file".into(),
                    size: entry.size,
                    compressed_size: entry.compressed_size,
                    modified_at: entry.modified_at.clone(),
                },
            );
        } else {
            // Direct child folder
            let dir_name = parts[0].to_string();
            let full_dir_path = if clean_subpath.is_empty() {
                dir_name.clone()
            } else {
                format!("{}/{}", clean_subpath, dir_name)
            };

            directories.entry(dir_name.clone()).or_insert_with(|| VirtualArchiveEntry {
                name: dir_name,
                path: full_dir_path,
                kind: "directory".into(),
                size: 0,
                compressed_size: None,
                modified_at: None,
            });
        }
    }

    let mut result: Vec<VirtualArchiveEntry> = directories.into_values().collect();
    result.extend(files.into_values());
    result
}

/// List virtual directory contents inside a ZIP or TAR.GZ archive with in-memory LRU caching
pub async fn list_virtual_archive_entries(
    provider: &Arc<dyn FileSystem>,
    archive_path: &VfsPath,
    subpath: &str,
) -> Result<Vec<VirtualArchiveEntry>, VfsError> {
    let format = ArchiveFormat::from_path(&archive_path.path).ok_or_else(|| {
        VfsError::IoError(format!("Unsupported archive format: {}", archive_path.path))
    })?;

    let meta = provider.stat(archive_path).await?;
    let cache_key = get_cache_key(archive_path);

    // 1. Check L1 Memory Index Cache for instantaneous O(1) response
    if let Ok(guard) = ARCHIVE_CACHE.read() {
        if let Some(cached) = guard.get(&cache_key) {
            if cached.size == meta.size && cached.mtime == meta.modified_at {
                return Ok(filter_entries_by_subpath(&cached.all_entries, subpath));
            }
        }
    }

    // 2. Cache miss: Stream archive to temp file and build complete flat index
    let temp_file = tempfile::NamedTempFile::new()
        .map_err(|e| VfsError::IoError(format!("Failed creating temp archive file: {}", e)))?;
    let temp_path = temp_file.path().to_path_buf();

    {
        let mut file = std::fs::File::create(&temp_path)
            .map_err(|e| VfsError::IoError(format!("Failed opening temp file: {}", e)))?;
        let reader = provider.read_stream(archive_path).await?;
        stream_async_to_sync_writer(reader, &mut file).await?;
    }

    let temp_path_clone = temp_path.clone();
    let all_entries = tokio::task::spawn_blocking(move || -> Result<Vec<VirtualArchiveEntry>, VfsError> {
        let mut entries = Vec::new();
        match format {
            ArchiveFormat::Zip => {
                let file = std::fs::File::open(&temp_path_clone)
                    .map_err(|e| VfsError::IoError(format!("Failed opening archive: {}", e)))?;
                let mut zip = ZipArchive::new(file)
                    .map_err(|e| VfsError::IoError(format!("Invalid zip archive: {}", e)))?;

                for i in 0..zip.len() {
                    let file = zip.by_index(i)
                        .map_err(|e| VfsError::IoError(format!("Failed reading zip index {}: {}", i, e)))?;

                    let raw_name = file.name().to_string();
                    if let Ok(safe_name) = validate_archive_entry_path(&raw_name) {
                        let is_dir = file.is_dir() || raw_name.ends_with('/');
                        entries.push(VirtualArchiveEntry {
                            name: safe_name.split('/').last().unwrap_or(&safe_name).to_string(),
                            path: safe_name,
                            kind: if is_dir { "directory".into() } else { "file".into() },
                            size: file.size(),
                            compressed_size: Some(file.compressed_size()),
                            modified_at: None,
                        });
                    }
                }
            }
            ArchiveFormat::TarGz => {
                let file = std::fs::File::open(&temp_path_clone)
                    .map_err(|e| VfsError::IoError(format!("Failed opening archive: {}", e)))?;
                let gz_decoder = GzDecoder::new(file);
                let mut archive = tar::Archive::new(gz_decoder);

                let tar_entries = archive
                    .entries()
                    .map_err(|e| VfsError::IoError(format!("Invalid tar archive: {}", e)))?;

                for entry_res in tar_entries {
                    let entry = entry_res
                        .map_err(|e| VfsError::IoError(format!("Tar entry read error: {}", e)))?;

                    let path_buf = entry
                        .path()
                        .map_err(|e| VfsError::IoError(format!("Invalid path in tar: {}", e)))?
                        .to_path_buf();

                    let raw_name = path_buf.to_string_lossy().to_string();
                    if let Ok(safe_name) = validate_archive_entry_path(&raw_name) {
                        let is_dir = entry.header().entry_type().is_dir() || raw_name.ends_with('/');
                        entries.push(VirtualArchiveEntry {
                            name: safe_name.split('/').last().unwrap_or(&safe_name).to_string(),
                            path: safe_name,
                            kind: if is_dir { "directory".into() } else { "file".into() },
                            size: entry.size(),
                            compressed_size: None,
                            modified_at: None,
                        });
                    }
                }
            }
        }
        Ok(entries)
    })
    .await
    .map_err(|e| VfsError::IoError(format!("Blocking index task panicked: {}", e)))??;

    let _ = tokio::fs::remove_file(&temp_path).await;

    // 3. Store flat index into memory cache (bounded to 100 archives)
    if let Ok(mut guard) = ARCHIVE_CACHE.write() {
        if guard.len() >= 100 {
            guard.clear();
        }
        guard.insert(
            cache_key,
            CachedArchiveIndex {
                _cached_at: Instant::now(),
                mtime: meta.modified_at,
                size: meta.size,
                all_entries: all_entries.clone(),
            },
        );
    }

    Ok(filter_entries_by_subpath(&all_entries, subpath))
}

/// Read a single file entry from an archive
pub async fn read_virtual_archive_entry(
    provider: &Arc<dyn FileSystem>,
    archive_path: &VfsPath,
    entry_path: &str,
) -> Result<(String, Vec<u8>), VfsError> {
    let format = ArchiveFormat::from_path(&archive_path.path).ok_or_else(|| {
        VfsError::IoError(format!("Unsupported archive format: {}", archive_path.path))
    })?;

    let temp_file = tempfile::NamedTempFile::new()
        .map_err(|e| VfsError::IoError(format!("Failed creating temp archive file: {}", e)))?;
    let temp_path = temp_file.path().to_path_buf();

    {
        let mut file = std::fs::File::create(&temp_path)
            .map_err(|e| VfsError::IoError(format!("Failed opening temp file: {}", e)))?;
        let reader = provider.read_stream(archive_path).await?;
        stream_async_to_sync_writer(reader, &mut file).await?;
    }

    let target_clean = entry_path.trim_matches('/').to_string();
    let file_name = target_clean.split('/').last().unwrap_or("file").to_string();
    let temp_path_clone = temp_path.clone();
    let target_clean_clone = target_clean.clone();

    const MAX_ENTRY_PREVIEW_SIZE: usize = 50 * 1024 * 1024; // 50 MB safety limit

    let content = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, VfsError> {
        match format {
            ArchiveFormat::Zip => {
                let file = std::fs::File::open(&temp_path_clone)
                    .map_err(|e| VfsError::IoError(format!("Failed opening archive: {}", e)))?;
                let mut zip = ZipArchive::new(file)
                    .map_err(|e| VfsError::IoError(format!("Invalid zip archive: {}", e)))?;

                for i in 0..zip.len() {
                    let mut file = zip.by_index(i)
                        .map_err(|e| VfsError::IoError(format!("Failed reading zip entry: {}", e)))?;

                    let raw_name = file.name().to_string();
                    if let Ok(safe_name) = validate_archive_entry_path(&raw_name) {
                        if safe_name == target_clean_clone {
                            if file.size() > MAX_ENTRY_PREVIEW_SIZE as u64 {
                                return Err(VfsError::QuotaExceeded(
                                    "Archive entry exceeds maximum preview size of 50 MB".into(),
                                ));
                            }
                            let mut buf = Vec::with_capacity(file.size() as usize);
                            file.read_to_end(&mut buf)
                                .map_err(|e| VfsError::IoError(format!("Failed reading zip content: {}", e)))?;
                            return Ok(buf);
                        }
                    }
                }
            }
            ArchiveFormat::TarGz => {
                let file = std::fs::File::open(&temp_path_clone)
                    .map_err(|e| VfsError::IoError(format!("Failed opening archive: {}", e)))?;
                let gz_decoder = GzDecoder::new(file);
                let mut archive = tar::Archive::new(gz_decoder);

                let entries = archive
                    .entries()
                    .map_err(|e| VfsError::IoError(format!("Invalid tar archive: {}", e)))?;

                for entry_res in entries {
                    let mut entry = entry_res
                        .map_err(|e| VfsError::IoError(format!("Tar entry read error: {}", e)))?;

                    let path_buf = entry
                        .path()
                        .map_err(|e| VfsError::IoError(format!("Invalid path in tar: {}", e)))?
                        .to_path_buf();

                    let raw_name = path_buf.to_string_lossy().to_string();
                    if let Ok(safe_name) = validate_archive_entry_path(&raw_name) {
                        if safe_name == target_clean_clone {
                            if entry.size() > MAX_ENTRY_PREVIEW_SIZE as u64 {
                                return Err(VfsError::QuotaExceeded(
                                    "Archive entry exceeds maximum preview size of 50 MB".into(),
                                ));
                            }
                            let mut buf = Vec::with_capacity(entry.size() as usize);
                            entry.read_to_end(&mut buf)
                                .map_err(|e| VfsError::IoError(format!("Failed reading tar content: {}", e)))?;
                            return Ok(buf);
                        }
                    }
                }
            }
        }

        Err(VfsError::NotFound(format!("Entry '{}' not found in archive", target_clean_clone)))
    })
    .await
    .map_err(|e| VfsError::IoError(format!("Blocking read task panicked: {}", e)))??;

    let _ = tokio::fs::remove_file(&temp_path).await;
    Ok((file_name, content))
}

/// Extract selected entries from an archive into target_dir with streaming disk staging
pub async fn extract_selected_archive_entries(
    provider: &Arc<dyn FileSystem>,
    archive_path: &VfsPath,
    target_dir: &str,
    selected_entries: &[String],
) -> Result<usize, VfsError> {
    let format = ArchiveFormat::from_path(&archive_path.path).ok_or_else(|| {
        VfsError::IoError(format!("Unsupported archive format: {}", archive_path.path))
    })?;

    let temp_file = tempfile::NamedTempFile::new()
        .map_err(|e| VfsError::IoError(format!("Failed creating temp archive file: {}", e)))?;
    let temp_path = temp_file.path().to_path_buf();

    {
        let mut file = std::fs::File::create(&temp_path)
            .map_err(|e| VfsError::IoError(format!("Failed opening temp file: {}", e)))?;
        let reader = provider.read_stream(archive_path).await?;
        stream_async_to_sync_writer(reader, &mut file).await?;
    }

    let selected_set: HashSet<String> = selected_entries
        .iter()
        .map(|s| s.trim_matches('/').to_string())
        .collect();

    let temp_dir_obj = tempfile::tempdir()
        .map_err(|e| VfsError::IoError(format!("Failed creating staging dir: {}", e)))?;
    let staging_root = temp_dir_obj.path().to_path_buf();

    let staging_clone = staging_root.clone();
    let temp_path_clone = temp_path.clone();

    #[derive(Clone)]
    struct StagedEntry {
        safe_name: String,
        is_dir: bool,
    }

    let staged_entries = tokio::task::spawn_blocking(move || -> Result<Vec<StagedEntry>, VfsError> {
        let mut list = Vec::new();
        match format {
            ArchiveFormat::Zip => {
                let file = std::fs::File::open(&temp_path_clone)
                    .map_err(|e| VfsError::IoError(format!("Failed opening archive: {}", e)))?;
                let mut zip = ZipArchive::new(file)
                    .map_err(|e| VfsError::IoError(format!("Invalid zip archive: {}", e)))?;

                for i in 0..zip.len() {
                    let mut file = zip.by_index(i)
                        .map_err(|e| VfsError::IoError(format!("Failed reading zip index: {}", e)))?;

                    let raw_name = file.name().to_string();
                    let safe_name = validate_archive_entry_path(&raw_name)?;

                    let is_selected = selected_set.contains(&safe_name)
                        || selected_set.iter().any(|prefix| safe_name.starts_with(&format!("{}/", prefix)));

                    if !is_selected {
                        continue;
                    }

                    let is_dir = file.is_dir() || raw_name.ends_with('/');
                    let dest_path = staging_clone.join(&safe_name);

                    if is_dir {
                        std::fs::create_dir_all(&dest_path)
                            .map_err(|e| VfsError::IoError(format!("Failed creating dir: {}", e)))?;
                    } else {
                        if let Some(parent) = dest_path.parent() {
                            std::fs::create_dir_all(parent)
                                .map_err(|e| VfsError::IoError(format!("Failed creating parent dir: {}", e)))?;
                        }
                        let mut out_file = std::fs::File::create(&dest_path)
                            .map_err(|e| VfsError::IoError(format!("Failed creating output file: {}", e)))?;
                        std::io::copy(&mut file, &mut out_file)
                            .map_err(|e| VfsError::IoError(format!("Failed decompressing {}: {}", safe_name, e)))?;
                    }

                    list.push(StagedEntry { safe_name, is_dir });
                }
            }
            ArchiveFormat::TarGz => {
                let file = std::fs::File::open(&temp_path_clone)
                    .map_err(|e| VfsError::IoError(format!("Failed opening archive: {}", e)))?;
                let gz_decoder = GzDecoder::new(file);
                let mut archive = tar::Archive::new(gz_decoder);

                let entries = archive
                    .entries()
                    .map_err(|e| VfsError::IoError(format!("Invalid tar archive: {}", e)))?;

                for entry_res in entries {
                    let mut entry = entry_res
                        .map_err(|e| VfsError::IoError(format!("Tar entry read error: {}", e)))?;

                    let path_buf = entry
                        .path()
                        .map_err(|e| VfsError::IoError(format!("Invalid path in tar: {}", e)))?
                        .to_path_buf();

                    let raw_name = path_buf.to_string_lossy().to_string();
                    let safe_name = validate_archive_entry_path(&raw_name)?;

                    let is_selected = selected_set.contains(&safe_name)
                        || selected_set.iter().any(|prefix| safe_name.starts_with(&format!("{}/", prefix)));

                    if !is_selected {
                        continue;
                    }

                    let is_dir = entry.header().entry_type().is_dir() || raw_name.ends_with('/');
                    let dest_path = staging_clone.join(&safe_name);

                    if is_dir {
                        std::fs::create_dir_all(&dest_path)
                            .map_err(|e| VfsError::IoError(format!("Failed creating dir: {}", e)))?;
                    } else {
                        if let Some(parent) = dest_path.parent() {
                            std::fs::create_dir_all(parent)
                                .map_err(|e| VfsError::IoError(format!("Failed creating parent dir: {}", e)))?;
                        }
                        let mut out_file = std::fs::File::create(&dest_path)
                            .map_err(|e| VfsError::IoError(format!("Failed creating output file: {}", e)))?;
                        std::io::copy(&mut entry, &mut out_file)
                            .map_err(|e| VfsError::IoError(format!("Failed reading tar content: {}", e)))?;
                    }

                    list.push(StagedEntry { safe_name, is_dir });
                }
            }
        }
        Ok(list)
    })
    .await
    .map_err(|e| VfsError::IoError(format!("Blocking extract selected panicked: {}", e)))??;

    let mut extracted_count = 0;
    for entry in staged_entries {
        let full_dest_path = if target_dir == "/" {
            format!("/{}", entry.safe_name.trim_start_matches('/'))
        } else {
            format!("{}/{}", target_dir.trim_end_matches('/'), entry.safe_name.trim_start_matches('/'))
        };

        let dest_vfs = VfsPath::new(&archive_path.connection_id, &full_dest_path);

        if entry.is_dir {
            let _ = provider.create_dir(&dest_vfs).await;
        } else {
            if let Some(parent) = dest_vfs.parent() {
                let _ = provider.create_dir(&parent).await;
            }

            let disk_file = staging_root.join(&entry.safe_name);
            let async_reader = tokio::fs::File::open(&disk_file)
                .await
                .map_err(|e| VfsError::IoError(format!("Failed opening staged file: {}", e)))?;
            provider.write_stream(&dest_vfs, Box::new(async_reader)).await?;
            extracted_count += 1;
        }
    }

    let _ = tokio::fs::remove_file(&temp_path).await;
    Ok(extracted_count)
}
