use crate::domain::{FileKind, VfsPath};
use crate::errors::{SecurityError, VfsError};
use crate::vfs::FileSystem;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::io::{Cursor, Read, Write};
use std::path::Path;
use std::sync::Arc;
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

/// Create a ZIP archive from selected relative paths inside a connection
pub async fn compress_zip(
    provider: &Arc<dyn FileSystem>,
    connection_id: &str,
    base_dir: &str,
    relative_paths: &[String],
    target_zip_path: &VfsPath,
) -> Result<(), VfsError> {
    let mut buffer = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut buffer);
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        for rel in relative_paths {
            let full_vfs = if base_dir == "/" {
                VfsPath::new(connection_id, format!("/{}", rel.trim_start_matches('/')))
            } else {
                VfsPath::new(connection_id, format!("{}/{}", base_dir.trim_end_matches('/'), rel.trim_start_matches('/')))
            };

            let meta = provider.stat(&full_vfs).await?;
            if meta.kind == FileKind::File {
                let mut reader = provider.read_stream(&full_vfs).await?;
                let mut content = Vec::new();
                reader.read_to_end(&mut content).await.map_err(|e| {
                    VfsError::IoError(format!("Failed to read file {}: {}", full_vfs.path, e))
                })?;

                zip.start_file(rel.trim_start_matches('/'), options)
                    .map_err(|e| VfsError::IoError(format!("Zip entry error: {}", e)))?;
                zip.write_all(&content)
                    .map_err(|e| VfsError::IoError(format!("Zip write error: {}", e)))?;
            }
        }
        zip.finish()
            .map_err(|e| VfsError::IoError(format!("Zip finalize error: {}", e)))?;
    }

    let bytes = buffer.into_inner();
    let cursor = Cursor::new(bytes);
    provider.write_stream(target_zip_path, Box::new(cursor)).await?;

    Ok(())
}

/// Extract a ZIP archive with strict Zip Slip path traversal protection
pub async fn extract_zip(
    provider: &Arc<dyn FileSystem>,
    archive_path: &VfsPath,
    target_dir: &str,
) -> Result<usize, VfsError> {
    let mut reader = provider.read_stream(archive_path).await?;
    let mut zip_bytes = Vec::new();
    reader.read_to_end(&mut zip_bytes).await.map_err(|e| {
        VfsError::IoError(format!("Failed to read archive: {}", e))
    })?;

    let mut files_to_write = Vec::new();
    {
        let cursor = Cursor::new(zip_bytes);
        let mut zip = ZipArchive::new(cursor)
            .map_err(|e| VfsError::IoError(format!("Invalid zip archive: {}", e)))?;

        const MAX_ENTRIES: usize = 10_000;
        const MAX_UNCOMPRESSED_BYTES: u64 = 10 * 1024 * 1024 * 1024; // 10 GB limit

        if zip.len() > MAX_ENTRIES {
            return Err(SecurityError::PathTraversal(format!(
                "Zip archive rejected: contains too many entries ({} > {})",
                zip.len(),
                MAX_ENTRIES
            ))
            .into());
        }

        let mut total_uncompressed_bytes = 0u64;

        for i in 0..zip.len() {
            let mut file = zip.by_index(i)
                .map_err(|e| VfsError::IoError(format!("Failed reading zip index {}: {}", i, e)))?;

            let raw_name = file.name().to_string();

            // Strict Zip Slip Check: component-based traversal, prefix, root, and null byte prevention
            let path_obj = Path::new(&raw_name);
            let has_traversal = raw_name.contains('\0')
                || path_obj.is_absolute()
                || path_obj.components().any(|c| {
                    matches!(
                        c,
                        std::path::Component::ParentDir
                            | std::path::Component::RootDir
                            | std::path::Component::Prefix(_)
                    )
                });

            if has_traversal {
                return Err(SecurityError::PathTraversal(format!(
                    "Zip Slip attempt detected: {}",
                    raw_name
                ))
                .into());
            }

            let is_dir = file.is_dir();
            let mut content = Vec::new();
            if !is_dir {
                total_uncompressed_bytes += file.size();
                if total_uncompressed_bytes > MAX_UNCOMPRESSED_BYTES {
                    return Err(SecurityError::PathTraversal(
                        "Zip bomb detected: exceeded max uncompressed size limit".into(),
                    )
                    .into());
                }

                file.read_to_end(&mut content)
                    .map_err(|e| VfsError::IoError(format!("Failed extracting {}: {}", raw_name, e)))?;
            }

            files_to_write.push((raw_name, is_dir, content));
        }
    } // ZipArchive dropped before any await points

    let mut extracted_count = 0;
    for (raw_name, is_dir, content) in files_to_write {
        let full_dest_path = if target_dir == "/" {
            format!("/{}", raw_name.trim_start_matches('/'))
        } else {
            format!("{}/{}", target_dir.trim_end_matches('/'), raw_name.trim_start_matches('/'))
        };

        let dest_vfs = VfsPath::new(&archive_path.connection_id, &full_dest_path);

        if is_dir {
            let _ = provider.create_dir(&dest_vfs).await;
        } else {
            if let Some(parent) = dest_vfs.parent() {
                let _ = provider.create_dir(&parent).await;
            }

            let cursor = Cursor::new(content);
            provider.write_stream(&dest_vfs, Box::new(cursor)).await?;
            extracted_count += 1;
        }
    }

    Ok(extracted_count)
}

/// Create a TAR.GZ archive
pub async fn compress_targz(
    provider: &Arc<dyn FileSystem>,
    connection_id: &str,
    base_dir: &str,
    relative_paths: &[String],
    target_tar_path: &VfsPath,
) -> Result<(), VfsError> {
    let mut gz_encoder = GzEncoder::new(Vec::new(), Compression::default());
    {
        let mut tar = tar::Builder::new(&mut gz_encoder);

        for rel in relative_paths {
            let full_vfs = if base_dir == "/" {
                VfsPath::new(connection_id, format!("/{}", rel.trim_start_matches('/')))
            } else {
                VfsPath::new(connection_id, format!("{}/{}", base_dir.trim_end_matches('/'), rel.trim_start_matches('/')))
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

                tar.append_data(&mut header, rel.trim_start_matches('/'), &content[..])
                    .map_err(|e| VfsError::IoError(format!("Tar append error: {}", e)))?;
            }
        }
        tar.finish()
            .map_err(|e| VfsError::IoError(format!("Tar finalize error: {}", e)))?;
    }

    let bytes = gz_encoder
        .finish()
        .map_err(|e| VfsError::IoError(format!("Gzip finalize error: {}", e)))?;

    let cursor = Cursor::new(bytes);
    provider.write_stream(target_tar_path, Box::new(cursor)).await?;

    Ok(())
}

/// Extract a TAR.GZ archive with strict path traversal protection
pub async fn extract_targz(
    provider: &Arc<dyn FileSystem>,
    archive_path: &VfsPath,
    target_dir: &str,
) -> Result<usize, VfsError> {
    let mut reader = provider.read_stream(archive_path).await?;
    let mut gz_bytes = Vec::new();
    reader.read_to_end(&mut gz_bytes).await.map_err(|e| {
        VfsError::IoError(format!("Failed to read tar.gz archive: {}", e))
    })?;

    let mut files_to_write = Vec::new();
    {
        let gz_decoder = GzDecoder::new(&gz_bytes[..]);
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

            if raw_name.contains('\0') || raw_name.contains("..") {
                return Err(SecurityError::PathTraversal(format!(
                    "Zip Slip attempt in tar: {}",
                    raw_name
                ))
                .into());
            }

            let is_dir = entry.header().entry_type().is_dir();
            let mut content = Vec::new();
            if !is_dir {
                entry.read_to_end(&mut content)
                    .map_err(|e| VfsError::IoError(format!("Failed reading tar content: {}", e)))?;
            }

            files_to_write.push((raw_name, is_dir, content));
        }
    } // tar::Archive dropped before any await points

    let mut extracted_count = 0;
    for (raw_name, is_dir, content) in files_to_write {
        let full_dest_path = if target_dir == "/" {
            format!("/{}", raw_name.trim_start_matches('/'))
        } else {
            format!("{}/{}", target_dir.trim_end_matches('/'), raw_name.trim_start_matches('/'))
        };

        let dest_vfs = VfsPath::new(&archive_path.connection_id, &full_dest_path);

        if is_dir {
            let _ = provider.create_dir(&dest_vfs).await;
        } else {
            if let Some(parent) = dest_vfs.parent() {
                let _ = provider.create_dir(&parent).await;
            }

            let cursor = Cursor::new(content);
            provider.write_stream(&dest_vfs, Box::new(cursor)).await?;
            extracted_count += 1;
        }
    }

    Ok(extracted_count)
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

/// List virtual directory contents inside a ZIP or TAR.GZ archive at the specified subpath
pub async fn list_virtual_archive_entries(
    provider: &Arc<dyn FileSystem>,
    archive_path: &VfsPath,
    subpath: &str,
) -> Result<Vec<VirtualArchiveEntry>, VfsError> {
    let format = ArchiveFormat::from_path(&archive_path.path).ok_or_else(|| {
        VfsError::IoError(format!("Unsupported archive format: {}", archive_path.path))
    })?;

    let mut reader = provider.read_stream(archive_path).await?;
    let mut archive_bytes = Vec::new();
    reader.read_to_end(&mut archive_bytes).await.map_err(|e| {
        VfsError::IoError(format!("Failed to read archive: {}", e))
    })?;

    let clean_subpath = subpath.trim_matches('/');
    let subpath_prefix = if clean_subpath.is_empty() {
        String::new()
    } else {
        format!("{}/", clean_subpath)
    };

    let mut directories = std::collections::BTreeMap::<String, VirtualArchiveEntry>::new();
    let mut files = std::collections::BTreeMap::<String, VirtualArchiveEntry>::new();

    match format {
        ArchiveFormat::Zip => {
            let cursor = Cursor::new(archive_bytes);
            let mut zip = ZipArchive::new(cursor)
                .map_err(|e| VfsError::IoError(format!("Invalid zip archive: {}", e)))?;

            for i in 0..zip.len() {
                let file = zip.by_index(i)
                    .map_err(|e| VfsError::IoError(format!("Failed reading zip index {}: {}", i, e)))?;

                let raw_name = file.name().to_string();
                let clean_name = raw_name.trim_matches('/').to_string();
                if clean_name.is_empty() {
                    continue;
                }

                // Check if this item is inside the requested subpath
                if !subpath_prefix.is_empty() && !clean_name.starts_with(&subpath_prefix) {
                    continue;
                }

                // Get relative remaining path after subpath prefix
                let remainder = if subpath_prefix.is_empty() {
                    &clean_name
                } else {
                    &clean_name[subpath_prefix.len()..]
                };

                if remainder.is_empty() {
                    continue;
                }

                let is_dir = file.is_dir() || raw_name.ends_with('/');
                let parts: Vec<&str> = remainder.split('/').collect();

                if parts.len() == 1 && !is_dir {
                    // Direct file
                    let file_name = parts[0].to_string();
                    let full_entry_path = if clean_subpath.is_empty() {
                        file_name.clone()
                    } else {
                        format!("{}/{}", clean_subpath, file_name)
                    };

                    files.insert(
                        file_name.clone(),
                        VirtualArchiveEntry {
                            name: file_name,
                            path: full_entry_path,
                            kind: "file".into(),
                            size: file.size(),
                            compressed_size: Some(file.compressed_size()),
                            modified_at: None,
                        },
                    );
                } else {
                    // Direct or indirect subdirectory
                    let dir_name = parts[0].to_string();
                    let full_dir_path = if clean_subpath.is_empty() {
                        dir_name.clone()
                    } else {
                        format!("{}/{}", clean_subpath, dir_name)
                    };

                    directories.entry(dir_name.clone()).or_insert_with(|| {
                        VirtualArchiveEntry {
                            name: dir_name,
                            path: full_dir_path,
                            kind: "directory".into(),
                            size: 0,
                            compressed_size: None,
                            modified_at: None,
                        }
                    });
                }
            }
        }
        ArchiveFormat::TarGz => {
            let cursor = Cursor::new(archive_bytes);
            let gz_decoder = GzDecoder::new(cursor);
            let mut archive = tar::Archive::new(gz_decoder);

            let entries = archive
                .entries()
                .map_err(|e| VfsError::IoError(format!("Invalid tar archive: {}", e)))?;

            for entry_res in entries {
                let entry = entry_res
                    .map_err(|e| VfsError::IoError(format!("Tar entry read error: {}", e)))?;

                let path_buf = entry
                    .path()
                    .map_err(|e| VfsError::IoError(format!("Invalid path in tar: {}", e)))?
                    .to_path_buf();

                let raw_name = path_buf.to_string_lossy().to_string();
                let clean_name = raw_name.trim_matches('/').to_string();
                if clean_name.is_empty() {
                    continue;
                }

                if !subpath_prefix.is_empty() && !clean_name.starts_with(&subpath_prefix) {
                    continue;
                }

                let remainder = if subpath_prefix.is_empty() {
                    &clean_name
                } else {
                    &clean_name[subpath_prefix.len()..]
                };

                if remainder.is_empty() {
                    continue;
                }

                let is_dir = entry.header().entry_type().is_dir() || raw_name.ends_with('/');
                let parts: Vec<&str> = remainder.split('/').collect();

                if parts.len() == 1 && !is_dir {
                    let file_name = parts[0].to_string();
                    let full_entry_path = if clean_subpath.is_empty() {
                        file_name.clone()
                    } else {
                        format!("{}/{}", clean_subpath, file_name)
                    };

                    files.insert(
                        file_name.clone(),
                        VirtualArchiveEntry {
                            name: file_name,
                            path: full_entry_path,
                            kind: "file".into(),
                            size: entry.header().size().unwrap_or(0),
                            compressed_size: None,
                            modified_at: None,
                        },
                    );
                } else {
                    let dir_name = parts[0].to_string();
                    let full_dir_path = if clean_subpath.is_empty() {
                        dir_name.clone()
                    } else {
                        format!("{}/{}", clean_subpath, dir_name)
                    };

                    directories.entry(dir_name.clone()).or_insert_with(|| {
                        VirtualArchiveEntry {
                            name: dir_name,
                            path: full_dir_path,
                            kind: "directory".into(),
                            size: 0,
                            compressed_size: None,
                            modified_at: None,
                        }
                    });
                }
            }
        }
    }

    let mut result: Vec<VirtualArchiveEntry> = directories.into_values().collect();
    result.extend(files.into_values());
    Ok(result)
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

    let mut reader = provider.read_stream(archive_path).await?;
    let mut archive_bytes = Vec::new();
    reader.read_to_end(&mut archive_bytes).await.map_err(|e| {
        VfsError::IoError(format!("Failed to read archive: {}", e))
    })?;

    let target_clean = entry_path.trim_matches('/').to_string();
    let file_name = target_clean.split('/').last().unwrap_or("file").to_string();

    match format {
        ArchiveFormat::Zip => {
            let cursor = Cursor::new(archive_bytes);
            let mut zip = ZipArchive::new(cursor)
                .map_err(|e| VfsError::IoError(format!("Invalid zip archive: {}", e)))?;

            for i in 0..zip.len() {
                let mut file = zip.by_index(i)
                    .map_err(|e| VfsError::IoError(format!("Failed reading zip entry: {}", e)))?;

                let current_name = file.name().trim_matches('/').to_string();
                if current_name == target_clean {
                    let mut content = Vec::new();
                    file.read_to_end(&mut content)
                        .map_err(|e| VfsError::IoError(format!("Failed reading zip content: {}", e)))?;
                    return Ok((file_name, content));
                }
            }
        }
        ArchiveFormat::TarGz => {
            let cursor = Cursor::new(archive_bytes);
            let gz_decoder = GzDecoder::new(cursor);
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

                let current_name = path_buf.to_string_lossy().trim_matches('/').to_string();
                if current_name == target_clean {
                    let mut content = Vec::new();
                    entry.read_to_end(&mut content)
                        .map_err(|e| VfsError::IoError(format!("Failed reading tar content: {}", e)))?;
                    return Ok((file_name, content));
                }
            }
        }
    }

    Err(VfsError::NotFound(format!("Entry '{}' not found in archive", entry_path)))
}

/// Extract selected entries from an archive into target_dir
pub async fn extract_selected_archive_entries(
    provider: &Arc<dyn FileSystem>,
    archive_path: &VfsPath,
    target_dir: &str,
    selected_entries: &[String],
) -> Result<usize, VfsError> {
    let format = ArchiveFormat::from_path(&archive_path.path).ok_or_else(|| {
        VfsError::IoError(format!("Unsupported archive format: {}", archive_path.path))
    })?;

    let mut reader = provider.read_stream(archive_path).await?;
    let mut archive_bytes = Vec::new();
    reader.read_to_end(&mut archive_bytes).await.map_err(|e| {
        VfsError::IoError(format!("Failed to read archive: {}", e))
    })?;

    let selected_set: std::collections::HashSet<String> = selected_entries
        .iter()
        .map(|s| s.trim_matches('/').to_string())
        .collect();

    let mut files_to_write = Vec::new();

    match format {
        ArchiveFormat::Zip => {
            let cursor = Cursor::new(archive_bytes);
            let mut zip = ZipArchive::new(cursor)
                .map_err(|e| VfsError::IoError(format!("Invalid zip archive: {}", e)))?;

            for i in 0..zip.len() {
                let mut file = zip.by_index(i)
                    .map_err(|e| VfsError::IoError(format!("Failed reading zip index: {}", e)))?;

                let raw_name = file.name().to_string();
                let clean_name = raw_name.trim_matches('/').to_string();

                let is_selected = selected_set.contains(&clean_name)
                    || selected_set.iter().any(|prefix| clean_name.starts_with(&format!("{}/", prefix)));

                if !is_selected {
                    continue;
                }

                // Zip slip check
                if raw_name.contains('\0') || raw_name.contains("..") {
                    return Err(SecurityError::PathTraversal(format!("Zip Slip detected: {}", raw_name)).into());
                }

                let is_dir = file.is_dir() || raw_name.ends_with('/');
                let mut content = Vec::new();
                if !is_dir {
                    file.read_to_end(&mut content)
                        .map_err(|e| VfsError::IoError(format!("Failed reading zip content: {}", e)))?;
                }
                files_to_write.push((clean_name, is_dir, content));
            }
        }
        ArchiveFormat::TarGz => {
            let cursor = Cursor::new(archive_bytes);
            let gz_decoder = GzDecoder::new(cursor);
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
                let clean_name = raw_name.trim_matches('/').to_string();

                let is_selected = selected_set.contains(&clean_name)
                    || selected_set.iter().any(|prefix| clean_name.starts_with(&format!("{}/", prefix)));

                if !is_selected {
                    continue;
                }

                if raw_name.contains('\0') || raw_name.contains("..") {
                    return Err(SecurityError::PathTraversal(format!("Zip Slip detected: {}", raw_name)).into());
                }

                let is_dir = entry.header().entry_type().is_dir() || raw_name.ends_with('/');
                let mut content = Vec::new();
                if !is_dir {
                    entry.read_to_end(&mut content)
                        .map_err(|e| VfsError::IoError(format!("Failed reading tar content: {}", e)))?;
                }
                files_to_write.push((clean_name, is_dir, content));
            }
        }
    }

    let mut extracted_count = 0;
    for (raw_name, is_dir, content) in files_to_write {
        let full_dest_path = if target_dir == "/" {
            format!("/{}", raw_name.trim_start_matches('/'))
        } else {
            format!("{}/{}", target_dir.trim_end_matches('/'), raw_name.trim_start_matches('/'))
        };

        let dest_vfs = VfsPath::new(&archive_path.connection_id, &full_dest_path);

        if is_dir {
            let _ = provider.create_dir(&dest_vfs).await;
        } else {
            if let Some(parent) = dest_vfs.parent() {
                let _ = provider.create_dir(&parent).await;
            }

            let cursor = Cursor::new(content);
            provider.write_stream(&dest_vfs, Box::new(cursor)).await?;
            extracted_count += 1;
        }
    }

    Ok(extracted_count)
}
