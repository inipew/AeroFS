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

        for i in 0..zip.len() {
            let mut file = zip.by_index(i)
                .map_err(|e| VfsError::IoError(format!("Failed reading zip index {}: {}", i, e)))?;

            let raw_name = file.name().to_string();

            // Strict Zip Slip Check: ensure no .. or null bytes in filename
            if raw_name.contains('\0') || raw_name.contains("..") {
                return Err(SecurityError::PathTraversal(format!(
                    "Zip Slip attempt detected: {}",
                    raw_name
                ))
                .into());
            }

            let clean_path = Path::new(&raw_name);
            if clean_path.is_absolute() {
                return Err(SecurityError::PathTraversal(format!(
                    "Absolute path in zip detected: {}",
                    raw_name
                ))
                .into());
            }

            let is_dir = file.is_dir();
            let mut content = Vec::new();
            if !is_dir {
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
