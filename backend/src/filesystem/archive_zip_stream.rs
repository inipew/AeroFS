use crate::domain::{FileKind, VfsPath};
use crate::errors::{SecurityError, VfsError};
use crate::vfs::FileSystem;
use futures::StreamExt;
use std::io::Write;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::sync::mpsc;
use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

const ARCHIVE_STREAM_CHUNK: usize = 64 * 1024;
const ARCHIVE_STREAM_CHANNEL_CAPACITY: usize = 8;
const MAX_ARCHIVE_FILES: usize = 50_000;

enum ArchiveWriteCommand {
    StartEntry { path: String },
    Data(Vec<u8>),
    EndEntry,
    Finish,
    Abort,
}

fn provider_staging_path(target: &VfsPath) -> Result<VfsPath, VfsError> {
    let name = target
        .path
        .rsplit('/')
        .find(|component| !component.is_empty())
        .ok_or_else(|| VfsError::InvalidPath("Archive destination must be a file path".into()))?;
    let parent = target
        .path
        .rfind('/')
        .map(|index| if index == 0 { "/" } else { &target.path[..index] })
        .unwrap_or("/");
    let staging_name = format!(".{name}.aerofs-archive-{}.tmp", Uuid::new_v4());
    let path = if parent == "/" {
        format!("/{staging_name}")
    } else {
        format!("{parent}/{staging_name}")
    };
    VfsPath::new(&target.connection_id, path)
}

async fn target_exists(provider: &Arc<dyn FileSystem>, target: &VfsPath) -> Result<bool, VfsError> {
    match provider.stat(target).await {
        Ok(_) => Ok(true),
        Err(VfsError::NotFound(_)) => Ok(false),
        Err(error) => Err(error),
    }
}

fn can_replace_atomically(provider: &Arc<dyn FileSystem>) -> bool {
    let capabilities = provider.capabilities();
    provider.is_local() && capabilities.atomic_rename
}

async fn send_reader_to_worker(
    mut reader: Box<dyn AsyncRead + Unpin + Send>,
    tx: &mpsc::Sender<ArchiveWriteCommand>,
) -> Result<(), VfsError> {
    let mut buffer = vec![0u8; ARCHIVE_STREAM_CHUNK];
    loop {
        let n = reader
            .read(&mut buffer)
            .await
            .map_err(|error| VfsError::IoError(format!("Archive source read error: {error}")))?;
        if n == 0 {
            return Ok(());
        }
        tx.send(ArchiveWriteCommand::Data(buffer[..n].to_vec()))
            .await
            .map_err(|_| VfsError::IoError("Zip compressor stopped unexpectedly".into()))?;
    }
}

async fn stream_archive_file(
    provider: &Arc<dyn FileSystem>,
    relative_path: String,
    full_vfs: &VfsPath,
    tx: &mpsc::Sender<ArchiveWriteCommand>,
) -> Result<(), VfsError> {
    tx.send(ArchiveWriteCommand::StartEntry {
        path: relative_path,
    })
    .await
    .map_err(|_| VfsError::IoError("Zip compressor stopped unexpectedly".into()))?;
    let reader = provider.read_stream(full_vfs).await?;
    send_reader_to_worker(reader, tx).await?;
    tx.send(ArchiveWriteCommand::EndEntry)
        .await
        .map_err(|_| VfsError::IoError("Zip compressor stopped unexpectedly".into()))?;
    Ok(())
}

async fn stream_archive_files(
    provider: &Arc<dyn FileSystem>,
    connection_id: &str,
    base_dir: &str,
    relative_paths: &[String],
    tx: &mpsc::Sender<ArchiveWriteCommand>,
) -> Result<(), VfsError> {
    let mut file_count = 0usize;

    for rel in relative_paths {
        let relative_path = rel.trim_start_matches('/').to_string();
        let full_vfs = if base_dir == "/" {
            VfsPath::new(connection_id, format!("/{relative_path}"))?
        } else {
            VfsPath::new(
                connection_id,
                format!("{}/{relative_path}", base_dir.trim_end_matches('/')),
            )?
        };

        let meta = provider.stat(&full_vfs).await?;
        if meta.kind == FileKind::File {
            if file_count >= MAX_ARCHIVE_FILES {
                return Err(SecurityError::PathTraversal(format!(
                    "Archive file count exceeded limit ({MAX_ARCHIVE_FILES})"
                ))
                .into());
            }
            file_count += 1;
            stream_archive_file(provider, relative_path, &full_vfs, tx).await?;
        } else if meta.kind == FileKind::Directory {
            let mut stack = vec![(relative_path, full_vfs)];
            while let Some((parent_rel, parent_vfs)) = stack.pop() {
                let mut stream = provider.list_stream(&parent_vfs).await?;
                while let Some(entry) = stream.next().await {
                    let entry = entry?;
                    if file_count >= MAX_ARCHIVE_FILES {
                        return Err(SecurityError::PathTraversal(format!(
                            "Archive file count exceeded limit ({MAX_ARCHIVE_FILES})"
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
                        file_count += 1;
                        stream_archive_file(provider, child_rel, &child_vfs, tx).await?;
                    }
                }
            }
        }
    }

    Ok(())
}

pub async fn compress_zip_streaming(
    provider: &Arc<dyn FileSystem>,
    connection_id: &str,
    base_dir: &str,
    relative_paths: &[String],
    target_zip_path: &VfsPath,
) -> Result<(), VfsError> {
    let temp_file = tempfile::NamedTempFile::new()
        .map_err(|error| VfsError::IoError(format!("Failed to create temp zip file: {error}")))?;
    let temp_path = temp_file.path().to_path_buf();
    let worker_path = temp_path.clone();
    let (tx, mut rx) = mpsc::channel::<ArchiveWriteCommand>(ARCHIVE_STREAM_CHANNEL_CAPACITY);

    let worker = tokio::task::spawn_blocking(move || -> Result<(), VfsError> {
        let file = std::fs::File::create(&worker_path)
            .map_err(|error| VfsError::IoError(format!("Failed opening temp zip: {error}")))?;
        let mut zip = ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let mut entry_active = false;

        loop {
            match rx.blocking_recv() {
                Some(ArchiveWriteCommand::StartEntry { path }) => {
                    if entry_active {
                        return Err(VfsError::IoError(
                            "Zip entry started before previous entry completed".into(),
                        ));
                    }
                    zip.start_file(path, options)
                        .map_err(|error| VfsError::IoError(format!("Zip entry error: {error}")))?;
                    entry_active = true;
                }
                Some(ArchiveWriteCommand::Data(data)) => {
                    if !entry_active {
                        return Err(VfsError::IoError("Zip data received without entry".into()));
                    }
                    zip.write_all(&data)
                        .map_err(|error| VfsError::IoError(format!("Zip write error: {error}")))?;
                }
                Some(ArchiveWriteCommand::EndEntry) => {
                    if !entry_active {
                        return Err(VfsError::IoError("Zip entry ended without active entry".into()));
                    }
                    entry_active = false;
                }
                Some(ArchiveWriteCommand::Finish) => {
                    if entry_active {
                        return Err(VfsError::IoError(
                            "Zip archive finalized with an incomplete entry".into(),
                        ));
                    }
                    zip.finish().map_err(|error| {
                        VfsError::IoError(format!("Zip finalize error: {error}"))
                    })?;
                    return Ok(());
                }
                Some(ArchiveWriteCommand::Abort) => return Ok(()),
                None => {
                    return Err(VfsError::IoError(
                        "Zip producer stopped before archive finalization".into(),
                    ));
                }
            }
        }
    });

    let producer = async {
        let result = async {
            stream_archive_files(provider, connection_id, base_dir, relative_paths, &tx).await?;
            tx.send(ArchiveWriteCommand::Finish)
                .await
                .map_err(|_| VfsError::IoError("Zip compressor stopped unexpectedly".into()))?;
            Ok::<(), VfsError>(())
        }
        .await;
        if result.is_err() {
            let _ = tx.send(ArchiveWriteCommand::Abort).await;
        }
        result
    };

    let producer_result = producer.await;
    drop(tx);
    let worker_result = worker
        .await
        .map_err(|error| VfsError::IoError(format!("Zip compressor task panicked: {error}")))?;
    worker_result?;
    producer_result?;

    let exists = target_exists(provider, target_zip_path).await?;
    let capabilities = provider.capabilities();
    let can_stage_commit = capabilities.atomic_rename && (!exists || can_replace_atomically(provider));

    if can_stage_commit {
        let staging = provider_staging_path(target_zip_path)?;
        let async_file = tokio::fs::File::open(&temp_path)
            .await
            .map_err(|error| VfsError::IoError(format!("Failed opening output zip: {error}")))?;
        let upload_result = provider.write_stream(&staging, Box::new(async_file)).await;
        if let Err(error) = upload_result {
            let _ = provider.delete(&staging).await;
            return Err(error);
        }
        let commit_result = provider.rename(&staging, target_zip_path).await;
        if commit_result.is_err() {
            let _ = provider.delete(&staging).await;
        }
        commit_result
    } else {
        let async_file = tokio::fs::File::open(&temp_path)
            .await
            .map_err(|error| VfsError::IoError(format!("Failed opening output zip: {error}")))?;
        provider.write_stream(target_zip_path, Box::new(async_file)).await
    }
}
