use crate::domain::{FileKind, VfsPath};
use crate::errors::{SecurityError, VfsError};
use crate::vfs::FileSystem;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, LazyLock, RwLock};
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use utoipa::ToSchema;
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

const ARCHIVE_STREAM_CHUNK: usize = 64 * 1024;
const ARCHIVE_STREAM_CHANNEL_CAPACITY: usize = 8;

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
        return Err(SecurityError::PathTraversal(
            "NUL byte detected in archive entry path".into(),
        ));
    }

    let path_obj = Path::new(raw_name);
    if path_obj.is_absolute() {
        return Err(SecurityError::PathTraversal(format!(
            "Absolute path in archive rejected: {}",
            raw_name
        )));
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

enum ArchiveWriteCommand {
    StartEntry { path: String, size: Option<u64> },
    Data(Vec<u8>),
    EndEntry { written: u64 },
    Finish,
}

async fn send_reader_to_archive_worker(
    mut reader: Box<dyn tokio::io::AsyncRead + Unpin + Send>,
    tx: &tokio::sync::mpsc::Sender<ArchiveWriteCommand>,
) -> Result<u64, VfsError> {
    let mut buffer = vec![0u8; ARCHIVE_STREAM_CHUNK];
    let mut total = 0u64;
    loop {
        let n = reader
            .read(&mut buffer)
            .await
            .map_err(|error| VfsError::IoError(format!("Archive source read error: {}", error)))?;
        if n == 0 {
            break;
        }
        total = total.saturating_add(n as u64);
        tx.send(ArchiveWriteCommand::Data(buffer[..n].to_vec()))
            .await
            .map_err(|_| {
                VfsError::IoError("Archive compressor stopped before input completed".into())
            })?;
    }
    Ok(total)
}

async fn download_archive_to_temp(
    provider: &Arc<dyn FileSystem>,
    archive_path: &VfsPath,
    temp_path: &Path,
) -> Result<u64, VfsError> {
    let mut reader = provider.read_stream(archive_path).await?;
    let mut file = tokio::fs::File::create(temp_path)
        .await
        .map_err(|error| VfsError::IoError(format!("Failed opening temp file: {}", error)))?;
    let copied = tokio::io::copy(&mut reader, &mut file)
        .await
        .map_err(|error| VfsError::IoError(format!("Failed writing temp archive: {}", error)))?;
    file.flush()
        .await
        .map_err(|error| VfsError::IoError(format!("Failed flushing temp archive: {}", error)))?;
    Ok(copied)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ArchiveOverwriteMode {
    #[default]
    Overwrite,
    Skip,
    KeepBoth,
}

async fn path_exists(
    provider: &Arc<dyn FileSystem>,
    path: &VfsPath,
) -> Result<bool, VfsError> {
    match provider.stat(path).await {
        Ok(_) => Ok(true),
        Err(VfsError::NotFound(_)) => Ok(false),
        Err(error) => Err(error),
    }
}

async fn ensure_directory(
    provider: &Arc<dyn FileSystem>,
    path: &VfsPath,
) -> Result<(), VfsError> {
    match provider.create_dir(path).await {
        Ok(()) => Ok(()),
        Err(create_error) => match provider.stat(path).await {
            Ok(metadata) if metadata.kind == FileKind::Directory => Ok(()),
            Ok(_) => Err(VfsError::IoError(format!(
                "Destination '{}' exists but is not a directory",
                path.path
            ))),
            Err(VfsError::NotFound(_)) => Err(create_error),
            Err(stat_error) => Err(stat_error),
        },
    }
}

async fn apply_committed_permissions(
    provider: &Arc<dyn FileSystem>,
    path: &VfsPath,
    permissions: Option<&str>,
    mutation: &str,
) -> Result<(), VfsError> {
    if let Some(permissions) = permissions {
        provider
            .set_permissions(path, permissions)
            .await
            .map_err(|error| {
                VfsError::IoError(format!(
                    "Archive {} '{}' was committed, but applying inherited permissions '{}' failed: {}. Filesystem mutation committed; recovery required",
                    mutation,
                    path.path,
                    permissions,
                    error
                ))
            })?;
    }
    Ok(())
}

/// Collect all files to be included in an archive, recursing through directories via list_stream
async fn collect_archive_files(
    provider: &Arc<dyn FileSystem>,
    connection_id: &str,
    base_dir: &str,
    relative_paths: &[String],
) -> Result<Vec<(String, VfsPath)>, VfsError> {
    use futures::StreamExt;
    let mut files = Vec::new();

    const MAX_ARCHIVE_FILES: usize = 50_000;
    for rel in relative_paths {
        let full_vfs = if base_dir == "/" {
            VfsPath::new(connection_id, format!("/{}", rel.trim_start_matches('/')))?
        } else {
            VfsPath::new(
                connection_id,
                format!(
                    "{}/{}",
                    base_dir.trim_end_matches('/'),
                    rel.trim_start_matches('/')
                ),
            )?
        };

        let meta = provider.stat(&full_vfs).await?;
        if meta.kind == FileKind::File {
            if files.len() >= MAX_ARCHIVE_FILES {
                return Err(SecurityError::PathTraversal(format!(
                    "Archive file count exceeded limit ({})",
                    MAX_ARCHIVE_FILES
                ))
                .into());
            }
            files.push((rel.trim_start_matches('/').to_string(), full_vfs));
        } else if meta.kind == FileKind::Directory {
            let mut stack = vec![(rel.trim_start_matches('/').to_string(), full_vfs)];
            while let Some((parent_rel, parent_vfs)) = stack.pop() {
                let mut stream = provider.list_stream(&parent_vfs).await?;
                while let Some(res) = stream.next().await {
                    let entry = res?;
                    if files.len() >= MAX_ARCHIVE_FILES {
                        return Err(SecurityError::PathTraversal(format!(
                            "Archive file count exceeded limit ({})",
                            MAX_ARCHIVE_FILES
                        ))
                        .into());
                    }
                    let child_rel = if parent_rel.is_empty() {
                        entry.name.clone()
                    } else {
                        format!("{}/{}", parent_rel, entry.name)
                    };
                    let child_vfs = VfsPath::new(connection_id, &entry.path)?;
                    if entry.kind == FileKind::Directory {
                        stack.push((child_rel, child_vfs));
                    } else {
                        files.push((child_rel, child_vfs));
                    }
                }
            }
        }
    }
    Ok(files)
}

/// Compress selected files into a ZIP archive via a bounded async-to-blocking bridge.
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
    let files_to_pack =
        collect_archive_files(provider, connection_id, base_dir, relative_paths).await?;

    let (tx, mut rx) =
        tokio::sync::mpsc::channel::<ArchiveWriteCommand>(ARCHIVE_STREAM_CHANNEL_CAPACITY);
    let worker_path = temp_path.clone();
    let worker = tokio::task::spawn_blocking(move || -> Result<(), VfsError> {
        let file = std::fs::File::create(&worker_path)
            .map_err(|e| VfsError::IoError(format!("Failed opening temp file: {}", e)))?;
        let mut zip = ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        loop {
            match rx.blocking_recv() {
                Some(ArchiveWriteCommand::StartEntry { path, .. }) => {
                    zip.start_file(path, options)
                        .map_err(|e| VfsError::IoError(format!("Zip entry error: {}", e)))?;
                }
                Some(ArchiveWriteCommand::Data(data)) => {
                    zip.write_all(&data)
                        .map_err(|e| VfsError::IoError(format!("Zip write error: {}", e)))?;
                }
                Some(ArchiveWriteCommand::EndEntry { .. }) => {}
                Some(ArchiveWriteCommand::Finish) => {
                    zip.finish()
                        .map_err(|e| VfsError::IoError(format!("Zip finalize error: {}", e)))?;
                    return Ok(());
                }
                None => {
                    return Err(VfsError::IoError(
                        "Zip producer stopped before archive finalization".into(),
                    ));
                }
            }
        }
    });

    let producer_result: Result<(), VfsError> = async {
        for (rel_path, full_vfs) in files_to_pack {
            tx.send(ArchiveWriteCommand::StartEntry {
                path: rel_path,
                size: None,
            })
            .await
            .map_err(|_| VfsError::IoError("Zip compressor stopped unexpectedly".into()))?;
            let reader = provider.read_stream(&full_vfs).await?;
            let written = send_reader_to_archive_worker(reader, &tx).await?;
            tx.send(ArchiveWriteCommand::EndEntry { written })
                .await
                .map_err(|_| VfsError::IoError("Zip compressor stopped unexpectedly".into()))?;
        }
        tx.send(ArchiveWriteCommand::Finish)
            .await
            .map_err(|_| VfsError::IoError("Zip compressor stopped unexpectedly".into()))?;
        Ok(())
    }
    .await;
    drop(tx);

    let worker_result = worker
        .await
        .map_err(|e| VfsError::IoError(format!("Zip compressor task panicked: {}", e)))?;
    if let Err(error) = worker_result {
        return Err(error);
    }
    producer_result?;

    let async_file = tokio::fs::File::open(&temp_path)
        .await
        .map_err(|e| VfsError::IoError(format!("Failed opening output zip file: {}", e)))?;
    provider
        .write_stream(target_zip_path, Box::new(async_file))
        .await?;

    let _ = tokio::fs::remove_file(&temp_path).await;
    Ok(())
}

/// Extract a ZIP archive with streaming decompression, overwrite collision handling, and strict Zip Slip protection
pub async fn extract_zip(
    provider: &Arc<dyn FileSystem>,
    archive_path: &VfsPath,
    target_dir: &str,
    overwrite_mode: ArchiveOverwriteMode,
) -> Result<(usize, usize), VfsError> {
    let temp_file = tempfile::NamedTempFile::new()
        .map_err(|e| VfsError::IoError(format!("Failed to create temp extract file: {}", e)))?;
    let temp_path = temp_file.path().to_path_buf();

    // 1. Stream archive to temp file using Tokio file I/O; no synchronous write on an async worker.
    download_archive_to_temp(provider, archive_path, &temp_path).await?;

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
                    std::fs::create_dir_all(parent).map_err(|e| {
                        VfsError::IoError(format!("Failed creating parent dir: {}", e))
                    })?;
                }

                total_uncompressed_bytes += zip_entry.size();
                if total_uncompressed_bytes > MAX_UNCOMPRESSED_BYTES {
                    return Err(SecurityError::PathTraversal(
                        "Zip bomb detected: exceeded max uncompressed size limit".into(),
                    )
                    .into());
                }

                let mut out_file = std::fs::File::create(&dest_on_disk).map_err(|e| {
                    VfsError::IoError(format!("Failed creating output file: {}", e))
                })?;
                std::io::copy(&mut zip_entry, &mut out_file).map_err(|e| {
                    VfsError::IoError(format!("Failed decompressing {}: {}", safe_name, e))
                })?;
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
    let mut skipped_count = 0;
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
        let dest_vfs = VfsPath::new(&archive_path.connection_id, &full_dest_path)?;

        if item.is_dir {
            let dir_perms = crate::domain::resolve_destination_permissions_strict(
                provider,
                &dest_vfs,
                true,
                crate::domain::PermissionInheritanceMode::InheritParent,
            )
            .await?;
            ensure_directory(provider, &dest_vfs).await?;
            apply_committed_permissions(
                provider,
                &dest_vfs,
                dir_perms.as_deref(),
                "directory",
            )
            .await?;
        } else {
            let exists = path_exists(provider, &dest_vfs).await?;
            let final_dest_vfs = if exists {
                match overwrite_mode {
                    ArchiveOverwriteMode::Skip => {
                        skipped_count += 1;
                        continue;
                    }
                    ArchiveOverwriteMode::Overwrite => dest_vfs,
                    ArchiveOverwriteMode::KeepBoth => {
                        let mut counter = 1;
                        let (stem, ext) = if let Some(idx) = dest_vfs.path.rfind('.') {
                            (&dest_vfs.path[..idx], &dest_vfs.path[idx..])
                        } else {
                            (dest_vfs.path.as_str(), "")
                        };
                        let mut candidate = format!("{} ({}){}", stem, counter, ext);
                        while let Ok(cand_vfs) =
                            VfsPath::new(&archive_path.connection_id, &candidate)
                        {
                            if !path_exists(provider, &cand_vfs).await? {
                                break;
                            }
                            counter += 1;
                            candidate = format!("{} ({}){}", stem, counter, ext);
                        }
                        VfsPath::new(&archive_path.connection_id, candidate)?
                    }
                }
            } else {
                dest_vfs
            };

            let file_perms = crate::domain::resolve_destination_permissions_strict(
                provider,
                &final_dest_vfs,
                false,
                crate::domain::PermissionInheritanceMode::InheritExistingOrParent,
            )
            .await?;
            if let Some(parent) = final_dest_vfs.parent() {
                ensure_directory(provider, &parent).await?;
            }
            let disk_file = staging_root.join(&item.rel_path);
            let async_reader = tokio::fs::File::open(&disk_file)
                .await
                .map_err(|e| VfsError::IoError(format!("Failed reading staged file: {}", e)))?;
            provider
                .write_stream(&final_dest_vfs, Box::new(async_reader))
                .await?;

            apply_committed_permissions(
                provider,
                &final_dest_vfs,
                file_perms.as_deref(),
                "file",
            )
            .await?;

            extracted_count += 1;
        }
    }

    let _ = tokio::fs::remove_file(&temp_path).await;
    Ok((extracted_count, skipped_count))
}

/// Compress selected files into a TAR.GZ archive via a bounded async-to-blocking bridge.
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
    let files_to_pack =
        collect_archive_files(provider, connection_id, base_dir, relative_paths).await?;

    let (tx, mut rx) =
        tokio::sync::mpsc::channel::<ArchiveWriteCommand>(ARCHIVE_STREAM_CHANNEL_CAPACITY);
    let worker_path = temp_path.clone();
    let worker = tokio::task::spawn_blocking(move || -> Result<(), VfsError> {
        let file = std::fs::File::create(&worker_path)
            .map_err(|e| VfsError::IoError(format!("Failed opening temp file: {}", e)))?;
        let mut enc = GzEncoder::new(file, Compression::default());
        let mut active_expected_size: Option<u64> = None;
        let mut active_written = 0u64;

        loop {
            match rx.blocking_recv() {
                Some(ArchiveWriteCommand::StartEntry { path, size }) => {
                    if active_expected_size.is_some() {
                        return Err(VfsError::IoError(
                            "Tar entry started before previous entry completed".into(),
                        ));
                    }
                    let expected_size = size.ok_or_else(|| {
                        VfsError::IoError("Tar entry missing expected size".into())
                    })?;
                    let mut header = tar::Header::new_gnu();
                    header
                        .set_path(&path)
                        .map_err(|e| VfsError::IoError(format!("Tar path error: {}", e)))?;
                    header.set_size(expected_size);
                    header.set_mode(0o644);
                    header.set_cksum();
                    enc.write_all(header.as_bytes()).map_err(|e| {
                        VfsError::IoError(format!("Tar header write error: {}", e))
                    })?;
                    active_expected_size = Some(expected_size);
                    active_written = 0;
                }
                Some(ArchiveWriteCommand::Data(data)) => {
                    if active_expected_size.is_none() {
                        return Err(VfsError::IoError("Tar data received without entry".into()));
                    }
                    enc.write_all(&data)
                        .map_err(|e| VfsError::IoError(format!("Tar content write error: {}", e)))?;
                    active_written = active_written.saturating_add(data.len() as u64);
                }
                Some(ArchiveWriteCommand::EndEntry { written }) => {
                    let expected_size = active_expected_size.take().ok_or_else(|| {
                        VfsError::IoError("Tar entry ended without active entry".into())
                    })?;
                    if written != active_written || written != expected_size {
                        return Err(VfsError::IoError(format!(
                            "Tar source size changed while archiving (expected {}, streamed {})",
                            expected_size, written
                        )));
                    }
                    let padding = (512 - (written % 512)) % 512;
                    if padding > 0 {
                        enc.write_all(&[0u8; 512][..padding as usize]).map_err(|e| {
                            VfsError::IoError(format!("Tar padding write error: {}", e))
                        })?;
                    }
                    active_written = 0;
                }
                Some(ArchiveWriteCommand::Finish) => {
                    if active_expected_size.is_some() {
                        return Err(VfsError::IoError(
                            "Tar archive finalized with an incomplete entry".into(),
                        ));
                    }
                    enc.write_all(&[0u8; 1024]).map_err(|e| {
                        VfsError::IoError(format!("Tar trailer write error: {}", e))
                    })?;
                    enc.finish()
                        .map_err(|e| VfsError::IoError(format!("Gzip finish error: {}", e)))?;
                    return Ok(());
                }
                None => {
                    return Err(VfsError::IoError(
                        "Tar producer stopped before archive finalization".into(),
                    ));
                }
            }
        }
    });

    let producer_result: Result<(), VfsError> = async {
        for (rel_path, full_vfs) in files_to_pack {
            let meta = provider.stat(&full_vfs).await?;
            tx.send(ArchiveWriteCommand::StartEntry {
                path: rel_path,
                size: Some(meta.size),
            })
            .await
            .map_err(|_| VfsError::IoError("Tar compressor stopped unexpectedly".into()))?;
            let reader = provider.read_stream(&full_vfs).await?;
            let written = send_reader_to_archive_worker(reader, &tx).await?;
            tx.send(ArchiveWriteCommand::EndEntry { written })
                .await
                .map_err(|_| VfsError::IoError("Tar compressor stopped unexpectedly".into()))?;
        }
        tx.send(ArchiveWriteCommand::Finish)
            .await
            .map_err(|_| VfsError::IoError("Tar compressor stopped unexpectedly".into()))?;
        Ok(())
    }
    .await;
    drop(tx);

    let worker_result = worker
        .await
        .map_err(|e| VfsError::IoError(format!("Tar compressor task panicked: {}", e)))?;
    if let Err(error) = worker_result {
        return Err(error);
    }
    producer_result?;

    let async_file = tokio::fs::File::open(&temp_path)
        .await
        .map_err(|e| VfsError::IoError(format!("Failed opening output targz file: {}", e)))?;
    provider
        .write_stream(target_targz_path, Box::new(async_file))
        .await?;

    let _ = tokio::fs::remove_file(&temp_path).await;
    Ok(())
}

/// Extract a TAR.GZ archive with streaming decompression, overwrite collision handling, and strict security checks
pub async fn extract_targz(
    provider: &Arc<dyn FileSystem>,
    archive_path: &VfsPath,
    target_dir: &str,
    overwrite_mode: ArchiveOverwriteMode,
) -> Result<(usize, usize), VfsError> {
    let temp_file = tempfile::NamedTempFile::new()
        .map_err(|e| VfsError::IoError(format!("Failed to create temp extract file: {}", e)))?;
    let temp_path = temp_file.path().to_path_buf();

    // 1. Stream archive to temp file using Tokio file I/O.
    download_archive_to_temp(provider, archive_path, &temp_path).await?;

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

            let mut entry =
                entry_res.map_err(|e| VfsError::IoError(format!("Tar entry read error: {}", e)))?;

            let entry_type = entry.header().entry_type();
            // Block special devices, fifos, block devices, symlinks pointing outside
            if entry_type.is_symlink()
                || entry_type.is_hard_link()
                || entry_type.is_fifo()
                || entry_type.is_character_special()
                || entry_type.is_block_special()
            {
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
                    std::fs::create_dir_all(parent).map_err(|e| {
                        VfsError::IoError(format!("Failed creating parent dir: {}", e))
                    })?;
                }

                let mut out_file = std::fs::File::create(&dest_on_disk).map_err(|e| {
                    VfsError::IoError(format!("Failed creating output file: {}", e))
                })?;
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
    let mut skipped_count = 0;
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

        let dest_vfs = VfsPath::new(&archive_path.connection_id, &full_dest_path)?;

        if item.is_dir {
            let dir_perms = crate::domain::resolve_destination_permissions_strict(
                provider,
                &dest_vfs,
                true,
                crate::domain::PermissionInheritanceMode::InheritParent,
            )
            .await?;
            ensure_directory(provider, &dest_vfs).await?;
            apply_committed_permissions(
                provider,
                &dest_vfs,
                dir_perms.as_deref(),
                "directory",
            )
            .await?;
        } else {
            let exists = path_exists(provider, &dest_vfs).await?;
            let final_dest_vfs = if exists {
                match overwrite_mode {
                    ArchiveOverwriteMode::Skip => {
                        skipped_count += 1;
                        continue;
                    }
                    ArchiveOverwriteMode::Overwrite => dest_vfs,
                    ArchiveOverwriteMode::KeepBoth => {
                        let mut counter = 1;
                        let (stem, ext) = if let Some(idx) = dest_vfs.path.rfind('.') {
                            (&dest_vfs.path[..idx], &dest_vfs.path[idx..])
                        } else {
                            (dest_vfs.path.as_str(), "")
                        };
                        let mut candidate = format!("{} ({}){}", stem, counter, ext);
                        while let Ok(cand_vfs) =
                            VfsPath::new(&archive_path.connection_id, &candidate)
                        {
                            if !path_exists(provider, &cand_vfs).await? {
                                break;
                            }
                            counter += 1;
                            candidate = format!("{} ({}){}", stem, counter, ext);
                        }
                        VfsPath::new(&archive_path.connection_id, candidate)?
                    }
                }
            } else {
                dest_vfs
            };

            let file_perms = crate::domain::resolve_destination_permissions_strict(
                provider,
                &final_dest_vfs,
                false,
                crate::domain::PermissionInheritanceMode::InheritExistingOrParent,
            )
            .await?;
            if let Some(parent) = final_dest_vfs.parent() {
                ensure_directory(provider, &parent).await?;
            }

            let disk_file = staging_root.join(&item.rel_path);
            let async_reader = tokio::fs::File::open(&disk_file)
                .await
                .map_err(|e| VfsError::IoError(format!("Failed reading staged file: {}", e)))?;
            provider
                .write_stream(&final_dest_vfs, Box::new(async_reader))
                .await?;

            apply_committed_permissions(
                provider,
                &final_dest_vfs,
                file_perms.as_deref(),
                "file",
            )
            .await?;

            extracted_count += 1;
        }
    }

    let _ = tokio::fs::remove_file(&temp_path).await;
    Ok((extracted_count, skipped_count))
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

            directories
                .entry(dir_name.clone())
                .or_insert_with(|| VirtualArchiveEntry {
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
    download_archive_to_temp(provider, archive_path, &temp_path).await?;

    let temp_path_clone = temp_path.clone();
    let all_entries =
        tokio::task::spawn_blocking(move || -> Result<Vec<VirtualArchiveEntry>, VfsError> {
            let mut entries = Vec::new();
            match format {
                ArchiveFormat::Zip => {
                    let file = std::fs::File::open(&temp_path_clone)
                        .map_err(|e| VfsError::IoError(format!("Failed opening archive: {}", e)))?;
                    let mut zip = ZipArchive::new(file)
                        .map_err(|e| VfsError::IoError(format!("Invalid zip archive: {}", e)))?;

                    for i in 0..zip.len() {
                        let file = zip.by_index(i).map_err(|e| {
                            VfsError::IoError(format!("Failed reading zip index {}: {}", i, e))
                        })?;

                        let raw_name = file.name().to_string();
                        if let Ok(safe_name) = validate_archive_entry_path(&raw_name) {
                            let is_dir = file.is_dir() || raw_name.ends_with('/');
                            entries.push(VirtualArchiveEntry {
                                name: safe_name
                                    .split('/')
                                    .next_back()
                                    .unwrap_or(&safe_name)
                                    .to_string(),
                                path: safe_name,
                                kind: if is_dir {
                                    "directory".into()
                                } else {
                                    "file".into()
                                },
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
                        let entry = entry_res.map_err(|e| {
                            VfsError::IoError(format!("Tar entry read error: {}", e))
                        })?;

                        let path_buf = entry
                            .path()
                            .map_err(|e| VfsError::IoError(format!("Invalid path in tar: {}", e)))?
                            .to_path_buf();

                        let raw_name = path_buf.to_string_lossy().to_string();
                        if let Ok(safe_name) = validate_archive_entry_path(&raw_name) {
                            let is_dir =
                                entry.header().entry_type().is_dir() || raw_name.ends_with('/');
                            entries.push(VirtualArchiveEntry {
                                name: safe_name
                                    .split('/')
                                    .next_back()
                                    .unwrap_or(&safe_name)
                                    .to_string(),
                                path: safe_name,
                                kind: if is_dir {
                                    "directory".into()
                                } else {
                                    "file".into()
                                },
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
    download_archive_to_temp(provider, archive_path, &temp_path).await?;

    let target_clean = entry_path.trim_matches('/').to_string();
    let file_name = target_clean
        .split('/')
        .next_back()
        .unwrap_or("file")
        .to_string();
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
                    let mut file = zip.by_index(i).map_err(|e| {
                        VfsError::IoError(format!("Failed reading zip entry: {}", e))
                    })?;

                    let raw_name = file.name().to_string();
                    if let Ok(safe_name) = validate_archive_entry_path(&raw_name) {
                        if safe_name == target_clean_clone {
                            if file.size() > MAX_ENTRY_PREVIEW_SIZE as u64 {
                                return Err(VfsError::QuotaExceeded(
                                    "Archive entry exceeds maximum preview size of 50 MB".into(),
                                ));
                            }
                            let mut buf = Vec::with_capacity(file.size() as usize);
                            file.read_to_end(&mut buf).map_err(|e| {
                                VfsError::IoError(format!("Failed reading zip content: {}", e))
                            })?;
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
                            entry.read_to_end(&mut buf).map_err(|e| {
                                VfsError::IoError(format!("Failed reading tar content: {}", e))
                            })?;
                            return Ok(buf);
                        }
                    }
                }
            }
        }

        Err(VfsError::NotFound(format!(
            "Entry '{}' not found in archive",
            target_clean_clone
        )))
    })
    .await
    .map_err(|e| VfsError::IoError(format!("Blocking read task panicked: {}", e)))??;

    let _ = tokio::fs::remove_file(&temp_path).await;
    Ok((file_name, content))
}

/// Extract selected entries from an archive into target_dir with streaming disk staging, overwrite collision handling, and permission inheritance
pub async fn extract_selected_archive_entries(
    provider: &Arc<dyn FileSystem>,
    archive_path: &VfsPath,
    target_dir: &str,
    selected_entries: &[String],
    overwrite_mode: ArchiveOverwriteMode,
) -> Result<(usize, usize), VfsError> {
    let format = ArchiveFormat::from_path(&archive_path.path).ok_or_else(|| {
        VfsError::IoError(format!("Unsupported archive format: {}", archive_path.path))
    })?;

    let temp_file = tempfile::NamedTempFile::new()
        .map_err(|e| VfsError::IoError(format!("Failed creating temp archive file: {}", e)))?;
    let temp_path = temp_file.path().to_path_buf();
    download_archive_to_temp(provider, archive_path, &temp_path).await?;

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

    let staged_entries =
        tokio::task::spawn_blocking(move || -> Result<Vec<StagedEntry>, VfsError> {
            let mut list = Vec::new();
            match format {
                ArchiveFormat::Zip => {
                    let file = std::fs::File::open(&temp_path_clone)
                        .map_err(|e| VfsError::IoError(format!("Failed opening archive: {}", e)))?;
                    let mut zip = ZipArchive::new(file)
                        .map_err(|e| VfsError::IoError(format!("Invalid zip archive: {}", e)))?;

                    for i in 0..zip.len() {
                        let mut file = zip.by_index(i).map_err(|e| {
                            VfsError::IoError(format!("Failed reading zip index: {}", e))
                        })?;

                        let raw_name = file.name().to_string();
                        let safe_name = validate_archive_entry_path(&raw_name)?;

                        let is_selected = selected_set.contains(&safe_name)
                            || selected_set
                                .iter()
                                .any(|prefix| safe_name.starts_with(&format!("{}/", prefix)));

                        if !is_selected {
                            continue;
                        }

                        let is_dir = file.is_dir() || raw_name.ends_with('/');
                        let dest_path = staging_clone.join(&safe_name);

                        if is_dir {
                            std::fs::create_dir_all(&dest_path).map_err(|e| {
                                VfsError::IoError(format!("Failed creating dir: {}", e))
                            })?;
                        } else {
                            if let Some(parent) = dest_path.parent() {
                                std::fs::create_dir_all(parent).map_err(|e| {
                                    VfsError::IoError(format!("Failed creating parent dir: {}", e))
                                })?;
                            }
                            let mut out_file = std::fs::File::create(&dest_path).map_err(|e| {
                                VfsError::IoError(format!("Failed creating output file: {}", e))
                            })?;
                            std::io::copy(&mut file, &mut out_file).map_err(|e| {
                                VfsError::IoError(format!(
                                    "Failed decompressing {}: {}",
                                    safe_name, e
                                ))
                            })?;
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
                        let mut entry = entry_res.map_err(|e| {
                            VfsError::IoError(format!("Tar entry read error: {}", e))
                        })?;

                        let path_buf = entry
                            .path()
                            .map_err(|e| VfsError::IoError(format!("Invalid path in tar: {}", e)))?
                            .to_path_buf();

                        let raw_name = path_buf.to_string_lossy().to_string();
                        let safe_name = validate_archive_entry_path(&raw_name)?;

                        let is_selected = selected_set.contains(&safe_name)
                            || selected_set
                                .iter()
                                .any(|prefix| safe_name.starts_with(&format!("{}/", prefix)));

                        if !is_selected {
                            continue;
                        }

                        let is_dir =
                            entry.header().entry_type().is_dir() || raw_name.ends_with('/');
                        let dest_path = staging_clone.join(&safe_name);

                        if is_dir {
                            std::fs::create_dir_all(&dest_path).map_err(|e| {
                                VfsError::IoError(format!("Failed creating dir: {}", e))
                            })?;
                        } else {
                            if let Some(parent) = dest_path.parent() {
                                std::fs::create_dir_all(parent).map_err(|e| {
                                    VfsError::IoError(format!("Failed creating parent dir: {}", e))
                                })?;
                            }
                            let mut out_file = std::fs::File::create(&dest_path).map_err(|e| {
                                VfsError::IoError(format!("Failed creating output file: {}", e))
                            })?;
                            std::io::copy(&mut entry, &mut out_file).map_err(|e| {
                                VfsError::IoError(format!("Failed reading tar content: {}", e))
                            })?;
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
    let mut skipped_count = 0;
    for entry in staged_entries {
        let full_dest_path = if target_dir == "/" {
            format!("/{}", entry.safe_name.trim_start_matches('/'))
        } else {
            format!(
                "{}/{}",
                target_dir.trim_end_matches('/'),
                entry.safe_name.trim_start_matches('/')
            )
        };

        let dest_vfs = VfsPath::new(&archive_path.connection_id, &full_dest_path)?;

        if entry.is_dir {
            let dir_perms = crate::domain::resolve_destination_permissions_strict(
                provider,
                &dest_vfs,
                true,
                crate::domain::PermissionInheritanceMode::InheritParent,
            )
            .await?;
            ensure_directory(provider, &dest_vfs).await?;
            apply_committed_permissions(
                provider,
                &dest_vfs,
                dir_perms.as_deref(),
                "directory",
            )
            .await?;
        } else {
            let exists = path_exists(provider, &dest_vfs).await?;
            let final_dest_vfs = if exists {
                match overwrite_mode {
                    ArchiveOverwriteMode::Skip => {
                        skipped_count += 1;
                        continue;
                    }
                    ArchiveOverwriteMode::Overwrite => dest_vfs,
                    ArchiveOverwriteMode::KeepBoth => {
                        let mut counter = 1;
                        let (stem, ext) = if let Some(idx) = dest_vfs.path.rfind('.') {
                            (&dest_vfs.path[..idx], &dest_vfs.path[idx..])
                        } else {
                            (dest_vfs.path.as_str(), "")
                        };
                        let mut candidate = format!("{} ({}){}", stem, counter, ext);
                        while let Ok(cand_vfs) =
                            VfsPath::new(&archive_path.connection_id, &candidate)
                        {
                            if !path_exists(provider, &cand_vfs).await? {
                                break;
                            }
                            counter += 1;
                            candidate = format!("{} ({}){}", stem, counter, ext);
                        }
                        VfsPath::new(&archive_path.connection_id, candidate)?
                    }
                }
            } else {
                dest_vfs
            };

            let file_perms = crate::domain::resolve_destination_permissions_strict(
                provider,
                &final_dest_vfs,
                false,
                crate::domain::PermissionInheritanceMode::InheritExistingOrParent,
            )
            .await?;
            if let Some(parent) = final_dest_vfs.parent() {
                ensure_directory(provider, &parent).await?;
            }

            let disk_file = staging_root.join(&entry.safe_name);
            let async_reader = tokio::fs::File::open(&disk_file)
                .await
                .map_err(|e| VfsError::IoError(format!("Failed opening staged file: {}", e)))?;
            provider
                .write_stream(&final_dest_vfs, Box::new(async_reader))
                .await?;

            apply_committed_permissions(
                provider,
                &final_dest_vfs,
                file_perms.as_deref(),
                "file",
            )
            .await?;

            extracted_count += 1;
        }
    }

    let _ = tokio::fs::remove_file(&temp_path).await;
    Ok((extracted_count, skipped_count))
}
