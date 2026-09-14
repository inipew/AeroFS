use crate::domain::{FileKind, PermissionInheritanceMode, VfsPath};
use crate::errors::{SecurityError, VfsError};
use crate::filesystem::archive::{validate_archive_entry_path, ArchiveOverwriteMode};
use crate::filesystem::archive_input_bridge::{input_pipe, pump_async_reader};
use crate::vfs::FileSystem;
use flate2::read::GzDecoder;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use zip::ZipArchive;

const EXTRACT_STREAM_CHUNK: usize = 64 * 1024;
const EXTRACT_STREAM_CHANNEL_CAPACITY: usize = 8;
const MAX_ARCHIVE_ENTRIES: usize = 50_000;
const MAX_UNCOMPRESSED_BYTES: u64 = 50 * 1024 * 1024 * 1024;

enum ExtractCommand {
    Directory {
        rel_path: String,
    },
    StartFile {
        rel_path: String,
        decision: tokio::sync::oneshot::Sender<bool>,
    },
    Data(Vec<u8>),
    EndFile,
    Finish,
}

struct ActiveFile {
    writer: Option<tokio::io::DuplexStream>,
    write_task: tokio::task::JoinHandle<Result<(), VfsError>>,
    destination: VfsPath,
    permissions: Option<String>,
}

async fn download_archive_to_temp(
    provider: &Arc<dyn FileSystem>,
    archive_path: &VfsPath,
    temp_path: &Path,
) -> Result<u64, VfsError> {
    let mut reader = provider.read_stream(archive_path).await?;
    let mut file = tokio::fs::File::create(temp_path)
        .await
        .map_err(|error| VfsError::IoError(format!("Failed opening temp archive: {error}")))?;
    let copied = tokio::io::copy(&mut reader, &mut file)
        .await
        .map_err(|error| VfsError::IoError(format!("Failed staging compressed archive: {error}")))?;
    file.flush()
        .await
        .map_err(|error| VfsError::IoError(format!("Failed flushing temp archive: {error}")))?;
    Ok(copied)
}

async fn path_exists(provider: &Arc<dyn FileSystem>, path: &VfsPath) -> Result<bool, VfsError> {
    match provider.stat(path).await {
        Ok(_) => Ok(true),
        Err(VfsError::NotFound(_)) => Ok(false),
        Err(error) => Err(error),
    }
}

async fn ensure_directory(provider: &Arc<dyn FileSystem>, path: &VfsPath) -> Result<(), VfsError> {
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

async fn apply_permissions(
    provider: &Arc<dyn FileSystem>,
    path: &VfsPath,
    permissions: Option<&str>,
) -> Result<(), VfsError> {
    if let Some(permissions) = permissions {
        provider
            .set_permissions(path, permissions)
            .await
            .map_err(|error| {
                VfsError::IoError(format!(
                    "Archive output '{}' was committed, but applying inherited permissions '{}' failed: {}. Filesystem mutation committed; recovery required",
                    path.path, permissions, error
                ))
            })?;
    }
    Ok(())
}

fn destination_path(
    connection_id: &str,
    target_dir: &str,
    rel_path: &str,
) -> Result<VfsPath, VfsError> {
    let full_path = if target_dir == "/" {
        format!("/{}", rel_path.trim_start_matches('/'))
    } else {
        format!(
            "{}/{}",
            target_dir.trim_end_matches('/'),
            rel_path.trim_start_matches('/')
        )
    };
    VfsPath::new(connection_id, full_path)
}

async fn resolve_file_destination(
    provider: &Arc<dyn FileSystem>,
    connection_id: &str,
    target_dir: &str,
    rel_path: &str,
    overwrite_mode: ArchiveOverwriteMode,
) -> Result<Option<(VfsPath, Option<String>)>, VfsError> {
    let destination = destination_path(connection_id, target_dir, rel_path)?;
    let exists = path_exists(provider, &destination).await?;
    let final_destination = if !exists {
        destination
    } else {
        match overwrite_mode {
            ArchiveOverwriteMode::Skip => return Ok(None),
            ArchiveOverwriteMode::Overwrite => destination,
            ArchiveOverwriteMode::KeepBoth => {
                let (stem, extension) = if let Some(index) = destination.path.rfind('.') {
                    (&destination.path[..index], &destination.path[index..])
                } else {
                    (destination.path.as_str(), "")
                };
                let mut counter = 1usize;
                loop {
                    let candidate = format!("{} ({}){}", stem, counter, extension);
                    let candidate_vfs = VfsPath::new(connection_id, candidate)?;
                    if !path_exists(provider, &candidate_vfs).await? {
                        break candidate_vfs;
                    }
                    counter = counter.saturating_add(1);
                }
            }
        }
    };

    let permissions = crate::domain::resolve_destination_permissions_strict(
        provider,
        &final_destination,
        false,
        PermissionInheritanceMode::InheritExistingOrParent,
    )
    .await?;
    if let Some(parent) = final_destination.parent() {
        ensure_directory(provider, &parent).await?;
    }
    Ok(Some((final_destination, permissions)))
}

async fn cleanup_active(active: &mut Option<ActiveFile>) {
    if let Some(mut file) = active.take() {
        file.writer.take();
        let _ = file.write_task.await;
    }
}

async fn consume_commands(
    provider: Arc<dyn FileSystem>,
    connection_id: &str,
    target_dir: &str,
    overwrite_mode: ArchiveOverwriteMode,
    mut rx: tokio::sync::mpsc::Receiver<ExtractCommand>,
) -> Result<(usize, usize), VfsError> {
    let mut extracted_count = 0usize;
    let mut skipped_count = 0usize;
    let mut active: Option<ActiveFile> = None;
    let mut saw_finish = false;

    let result: Result<(), VfsError> = async {
        while let Some(command) = rx.recv().await {
            match command {
                ExtractCommand::Directory { rel_path } => {
                    if active.is_some() {
                        return Err(VfsError::IoError(
                            "Archive extractor started a directory while a file was active".into(),
                        ));
                    }
                    let destination = destination_path(connection_id, target_dir, &rel_path)?;
                    let permissions = crate::domain::resolve_destination_permissions_strict(
                        &provider,
                        &destination,
                        true,
                        PermissionInheritanceMode::InheritParent,
                    )
                    .await?;
                    ensure_directory(&provider, &destination).await?;
                    apply_permissions(&provider, &destination, permissions.as_deref()).await?;
                }
                ExtractCommand::StartFile { rel_path, decision } => {
                    if active.is_some() {
                        return Err(VfsError::IoError(
                            "Archive extractor started a file before the previous file ended".into(),
                        ));
                    }
                    let resolved = resolve_file_destination(
                        &provider,
                        connection_id,
                        target_dir,
                        &rel_path,
                        overwrite_mode,
                    )
                    .await?;
                    let Some((destination, permissions)) = resolved else {
                        skipped_count = skipped_count.saturating_add(1);
                        let _ = decision.send(false);
                        continue;
                    };

                    let (writer, reader) = tokio::io::duplex(EXTRACT_STREAM_CHUNK);
                    let provider_for_write = Arc::clone(&provider);
                    let destination_for_write = destination.clone();
                    let write_task = tokio::spawn(async move {
                        provider_for_write
                            .write_stream(&destination_for_write, Box::new(reader))
                            .await
                    });
                    active = Some(ActiveFile {
                        writer: Some(writer),
                        write_task,
                        destination,
                        permissions,
                    });
                    if decision.send(true).is_err() {
                        return Err(VfsError::IoError(
                            "Archive extraction worker stopped before file streaming began".into(),
                        ));
                    }
                }
                ExtractCommand::Data(data) => {
                    let file = active.as_mut().ok_or_else(|| {
                        VfsError::IoError("Archive data arrived without an active file".into())
                    })?;
                    file.writer
                        .as_mut()
                        .ok_or_else(|| VfsError::IoError("Archive output stream is closed".into()))?
                        .write_all(&data)
                        .await
                        .map_err(|error| {
                            VfsError::IoError(format!("Failed streaming extracted data: {error}"))
                        })?;
                }
                ExtractCommand::EndFile => {
                    let mut file = active.take().ok_or_else(|| {
                        VfsError::IoError("Archive file ended without an active output".into())
                    })?;
                    if let Some(mut writer) = file.writer.take() {
                        writer.flush().await.map_err(|error| {
                            VfsError::IoError(format!("Failed flushing extracted data: {error}"))
                        })?;
                        drop(writer);
                    }
                    file.write_task
                        .await
                        .map_err(|error| {
                            VfsError::IoError(format!("Archive provider write task panicked: {error}"))
                        })??;
                    apply_permissions(
                        &provider,
                        &file.destination,
                        file.permissions.as_deref(),
                    )
                    .await?;
                    extracted_count = extracted_count.saturating_add(1);
                }
                ExtractCommand::Finish => {
                    if active.is_some() {
                        return Err(VfsError::IoError(
                            "Archive extraction finished with an active file".into(),
                        ));
                    }
                    saw_finish = true;
                    break;
                }
            }
        }
        if !saw_finish {
            return Err(VfsError::IoError(
                "Archive extraction worker stopped before finalization".into(),
            ));
        }
        Ok(())
    }
    .await;

    if result.is_err() {
        cleanup_active(&mut active).await;
    }
    result?;
    Ok((extracted_count, skipped_count))
}

fn send_command(
    tx: &tokio::sync::mpsc::Sender<ExtractCommand>,
    command: ExtractCommand,
) -> Result<(), VfsError> {
    tx.blocking_send(command).map_err(|_| {
        VfsError::IoError("Archive extraction consumer stopped unexpectedly".into())
    })
}

fn stream_reader<R: Read>(
    reader: &mut R,
    tx: &tokio::sync::mpsc::Sender<ExtractCommand>,
    total_streamed: &mut u64,
) -> Result<(), VfsError> {
    let mut buffer = vec![0u8; EXTRACT_STREAM_CHUNK];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| VfsError::IoError(format!("Archive decompression failed: {error}")))?;
        if read == 0 {
            break;
        }
        *total_streamed = total_streamed.saturating_add(read as u64);
        if *total_streamed > MAX_UNCOMPRESSED_BYTES {
            return Err(SecurityError::PathTraversal(
                "Archive decompression size limit exceeded (50 GB)".into(),
            )
            .into());
        }
        send_command(tx, ExtractCommand::Data(buffer[..read].to_vec()))?;
    }
    Ok(())
}

fn run_zip_worker(
    temp_path: &Path,
    tx: tokio::sync::mpsc::Sender<ExtractCommand>,
) -> Result<(), VfsError> {
    let file = std::fs::File::open(temp_path)
        .map_err(|error| VfsError::IoError(format!("Failed opening staged archive: {error}")))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| VfsError::IoError(format!("Invalid zip archive: {error}")))?;
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err(SecurityError::PathTraversal(format!(
            "Zip archive rejected: contains too many entries ({} > {})",
            archive.len(), MAX_ARCHIVE_ENTRIES
        ))
        .into());
    }

    let mut total_declared = 0u64;
    let mut total_streamed = 0u64;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| VfsError::IoError(format!("Failed reading zip index {index}: {error}")))?;
        let raw_name = entry.name().to_string();
        let safe_name = validate_archive_entry_path(&raw_name)?;
        if entry.is_dir() {
            send_command(&tx, ExtractCommand::Directory { rel_path: safe_name })?;
            continue;
        }

        total_declared = total_declared.saturating_add(entry.size());
        if total_declared > MAX_UNCOMPRESSED_BYTES {
            return Err(SecurityError::PathTraversal(
                "Zip bomb detected: exceeded max uncompressed size limit".into(),
            )
            .into());
        }

        let (decision_tx, decision_rx) = tokio::sync::oneshot::channel();
        send_command(
            &tx,
            ExtractCommand::StartFile {
                rel_path: safe_name,
                decision: decision_tx,
            },
        )?;
        let should_stream = decision_rx.blocking_recv().map_err(|_| {
            VfsError::IoError("Archive extraction consumer stopped before overwrite decision".into())
        })?;
        if should_stream {
            stream_reader(&mut entry, &tx, &mut total_streamed)?;
            send_command(&tx, ExtractCommand::EndFile)?;
        }
    }
    send_command(&tx, ExtractCommand::Finish)
}

fn run_targz_worker<R: Read>(
    reader: R,
    tx: tokio::sync::mpsc::Sender<ExtractCommand>,
) -> Result<(), VfsError> {
    let decoder = GzDecoder::new(reader);
    let mut archive = tar::Archive::new(decoder);
    let entries = archive
        .entries()
        .map_err(|error| VfsError::IoError(format!("Invalid tar archive: {error}")))?;

    let mut entry_count = 0usize;
    let mut total_declared = 0u64;
    let mut total_streamed = 0u64;
    for entry_result in entries {
        entry_count = entry_count.saturating_add(1);
        if entry_count > MAX_ARCHIVE_ENTRIES {
            return Err(SecurityError::PathTraversal(format!(
                "Tar archive rejected: contains too many entries (> {})",
                MAX_ARCHIVE_ENTRIES
            ))
            .into());
        }
        let mut entry = entry_result
            .map_err(|error| VfsError::IoError(format!("Tar entry read error: {error}")))?;
        let entry_type = entry.header().entry_type();
        if entry_type.is_symlink()
            || entry_type.is_hard_link()
            || entry_type.is_fifo()
            || entry_type.is_character_special()
            || entry_type.is_block_special()
        {
            continue;
        }

        let raw_name = entry
            .path()
            .map_err(|error| VfsError::IoError(format!("Invalid path in tar: {error}")))?
            .to_string_lossy()
            .to_string();
        let safe_name = validate_archive_entry_path(&raw_name)?;
        if entry_type.is_dir() {
            send_command(&tx, ExtractCommand::Directory { rel_path: safe_name })?;
            continue;
        }

        total_declared = total_declared.saturating_add(entry.size());
        if total_declared > MAX_UNCOMPRESSED_BYTES {
            return Err(SecurityError::PathTraversal(
                "Tar archive decompression size limit exceeded (50 GB)".into(),
            )
            .into());
        }

        let (decision_tx, decision_rx) = tokio::sync::oneshot::channel();
        send_command(
            &tx,
            ExtractCommand::StartFile {
                rel_path: safe_name,
                decision: decision_tx,
            },
        )?;
        let should_stream = decision_rx.blocking_recv().map_err(|_| {
            VfsError::IoError("Archive extraction consumer stopped before overwrite decision".into())
        })?;
        if should_stream {
            stream_reader(&mut entry, &tx, &mut total_streamed)?;
            send_command(&tx, ExtractCommand::EndFile)?;
        } else {
            std::io::copy(&mut entry, &mut std::io::sink()).map_err(|error| {
                VfsError::IoError(format!("Failed skipping tar entry content: {error}"))
            })?;
        }
    }
    send_command(&tx, ExtractCommand::Finish)
}

async fn extract_with_worker<F>(
    provider: &Arc<dyn FileSystem>,
    archive_path: &VfsPath,
    target_dir: &str,
    overwrite_mode: ArchiveOverwriteMode,
    worker_fn: F,
) -> Result<(usize, usize), VfsError>
where
    F: FnOnce(&Path, tokio::sync::mpsc::Sender<ExtractCommand>) -> Result<(), VfsError>
        + Send
        + 'static,
{
    let temp_file = tempfile::NamedTempFile::new()
        .map_err(|error| VfsError::IoError(format!("Failed creating temp archive: {error}")))?;
    let temp_path = temp_file.path().to_path_buf();
    download_archive_to_temp(provider, archive_path, &temp_path).await?;

    let (tx, rx) = tokio::sync::mpsc::channel(EXTRACT_STREAM_CHANNEL_CAPACITY);
    let worker_path = temp_path.clone();
    let worker = tokio::task::spawn_blocking(move || worker_fn(&worker_path, tx));
    let consumer_result = consume_commands(
        Arc::clone(provider),
        &archive_path.connection_id,
        target_dir,
        overwrite_mode,
        rx,
    )
    .await;
    let worker_result = worker
        .await
        .map_err(|error| VfsError::IoError(format!("Archive extraction task panicked: {error}")))?;

    match (consumer_result, worker_result) {
        (Err(consumer_error), _) => Err(consumer_error),
        (Ok(_), Err(worker_error)) => Err(worker_error),
        (Ok(counts), Ok(())) => Ok(counts),
    }
}

pub async fn extract_zip_streaming(
    provider: &Arc<dyn FileSystem>,
    archive_path: &VfsPath,
    target_dir: &str,
    overwrite_mode: ArchiveOverwriteMode,
) -> Result<(usize, usize), VfsError> {
    extract_with_worker(
        provider,
        archive_path,
        target_dir,
        overwrite_mode,
        |path, tx| run_zip_worker(path, tx),
    )
    .await
}

pub async fn extract_targz_streaming(
    provider: &Arc<dyn FileSystem>,
    archive_path: &VfsPath,
    target_dir: &str,
    overwrite_mode: ArchiveOverwriteMode,
) -> Result<(usize, usize), VfsError> {
    let source = provider.read_stream(archive_path).await?;
    let (input_tx, input_reader) = input_pipe();
    let (command_tx, command_rx) =
        tokio::sync::mpsc::channel(EXTRACT_STREAM_CHANNEL_CAPACITY);

    let worker = tokio::task::spawn_blocking(move || run_targz_worker(input_reader, command_tx));
    let producer = pump_async_reader(source, input_tx);
    let consumer = consume_commands(
        Arc::clone(provider),
        &archive_path.connection_id,
        target_dir,
        overwrite_mode,
        command_rx,
    );

    let (producer_result, consumer_result) = tokio::join!(producer, consumer);
    let worker_result = worker
        .await
        .map_err(|error| VfsError::IoError(format!("Archive extraction task panicked: {error}")))?;

    match (consumer_result, worker_result, producer_result) {
        (Err(consumer_error), _, _) => Err(consumer_error),
        (Ok(_), Err(worker_error), _) => Err(worker_error),
        (Ok(_), Ok(()), Err(producer_error)) => Err(producer_error),
        (Ok(counts), Ok(()), Ok(())) => Ok(counts),
    }
}
