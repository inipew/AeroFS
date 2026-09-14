use crate::domain::VfsPath;
use crate::errors::{SecurityError, VfsError};
use crate::filesystem::archive::{validate_archive_entry_path, ArchiveOverwriteMode};
use crate::filesystem::archive_extract_core::{
    consume_commands, request_file_stream, send_command, stream_reader, ExtractCommand,
    EXTRACT_STREAM_CHANNEL_CAPACITY, MAX_ARCHIVE_ENTRIES, MAX_UNCOMPRESSED_BYTES,
};
use crate::filesystem::archive_input_bridge::{input_pipe, pump_async_reader};
use crate::vfs::FileSystem;
use flate2::read::GzDecoder;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use zip::ZipArchive;

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

fn run_zip_worker(
    temp_path: &Path,
    tx: tokio::sync::mpsc::Sender<ExtractCommand>,
) -> Result<(), VfsError> {
    let file = std::fs::File::open(temp_path)
        .map_err(|error| VfsError::IoError(format!("Failed opening staged archive: {error}")))?;
    run_zip_reader(file, tx)
}

pub(crate) fn run_zip_reader<R: Read + std::io::Seek>(
    reader: R,
    tx: tokio::sync::mpsc::Sender<ExtractCommand>,
) -> Result<(), VfsError> {
    let mut archive = ZipArchive::new(reader)
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

        if request_file_stream(&tx, safe_name)? {
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

        if request_file_stream(&tx, safe_name)? {
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
