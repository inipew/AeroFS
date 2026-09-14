use crate::domain::{FileKind, PermissionInheritanceMode, VfsPath};
use crate::errors::{SecurityError, VfsError};
use crate::filesystem::archive::{validate_archive_entry_path, ArchiveOverwriteMode};
use crate::filesystem::archive_stream::extract_zip_streaming;
use crate::vfs::FileSystem;
use std::collections::VecDeque;
use std::io::{self, Read, Seek, SeekFrom};
use std::sync::{mpsc as std_mpsc, Arc};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use zip::ZipArchive;

const EXTRACT_STREAM_CHUNK: usize = 64 * 1024;
const EXTRACT_STREAM_CHANNEL_CAPACITY: usize = 8;
const MAX_ARCHIVE_ENTRIES: usize = 50_000;
const MAX_UNCOMPRESSED_BYTES: u64 = 50 * 1024 * 1024 * 1024;
pub(crate) const ZIP_RANGE_CHUNK_SIZE: u64 = 2 * 1024 * 1024;
pub(crate) const ZIP_RANGE_CACHE_CHUNKS: usize = 4;
pub(crate) const ZIP_RANGE_MIN_ARCHIVE_SIZE: u64 = 16 * 1024 * 1024;

struct RangeRequest {
    offset: u64,
    length: u64,
    response: std_mpsc::SyncSender<Result<Vec<u8>, String>>,
}

struct BlockingRangeReader {
    requests: mpsc::Sender<RangeRequest>,
    len: u64,
    position: u64,
    cache: VecDeque<(u64, Vec<u8>)>,
}

impl BlockingRangeReader {
    fn new(requests: mpsc::Sender<RangeRequest>, len: u64) -> Self {
        Self {
            requests,
            len,
            position: 0,
            cache: VecDeque::with_capacity(ZIP_RANGE_CACHE_CHUNKS),
        }
    }

    fn load_chunk(&mut self, chunk_start: u64) -> io::Result<()> {
        if let Some(index) = self
            .cache
            .iter()
            .position(|(offset, _)| *offset == chunk_start)
        {
            if let Some(entry) = self.cache.remove(index) {
                self.cache.push_back(entry);
            }
            return Ok(());
        }

        let remaining = self.len.saturating_sub(chunk_start);
        if remaining == 0 {
            return Ok(());
        }
        let length = remaining.min(ZIP_RANGE_CHUNK_SIZE);
        let (response_tx, response_rx) = std_mpsc::sync_channel(1);
        self.requests
            .blocking_send(RangeRequest {
                offset: chunk_start,
                length,
                response: response_tx,
            })
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "ZIP range request server stopped unexpectedly",
                )
            })?;

        let data = response_rx
            .recv()
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "ZIP range response channel closed unexpectedly",
                )
            })?
            .map_err(io::Error::other)?;

        if data.len() != length as usize {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!(
                    "ZIP range read returned {} bytes, expected {length}",
                    data.len()
                ),
            ));
        }

        if self.cache.len() == ZIP_RANGE_CACHE_CHUNKS {
            self.cache.pop_front();
        }
        self.cache.push_back((chunk_start, data));
        Ok(())
    }

    fn current_chunk(&self, chunk_start: u64) -> Option<&[u8]> {
        self.cache
            .iter()
            .rev()
            .find(|(offset, _)| *offset == chunk_start)
            .map(|(_, data)| data.as_slice())
    }
}

impl Read for BlockingRangeReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() || self.position >= self.len {
            return Ok(0);
        }

        let chunk_start = (self.position / ZIP_RANGE_CHUNK_SIZE) * ZIP_RANGE_CHUNK_SIZE;
        self.load_chunk(chunk_start)?;
        let chunk = self.current_chunk(chunk_start).ok_or_else(|| {
            io::Error::other("ZIP range cache lost the requested chunk")
        })?;
        let in_chunk = (self.position - chunk_start) as usize;
        if in_chunk >= chunk.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "ZIP range cursor exceeded cached chunk",
            ));
        }
        let read = buf.len().min(chunk.len() - in_chunk);
        buf[..read].copy_from_slice(&chunk[in_chunk..in_chunk + read]);
        self.position = self.position.saturating_add(read as u64);
        Ok(read)
    }
}

impl Seek for BlockingRangeReader {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        let next = match position {
            SeekFrom::Start(offset) => i128::from(offset),
            SeekFrom::End(delta) => i128::from(self.len) + i128::from(delta),
            SeekFrom::Current(delta) => i128::from(self.position) + i128::from(delta),
        };
        if next < 0 || next > i128::from(u64::MAX) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid ZIP seek target",
            ));
        }
        self.position = next as u64;
        Ok(self.position)
    }
}

async fn serve_range_requests(
    provider: Arc<dyn FileSystem>,
    archive_path: VfsPath,
    mut requests: mpsc::Receiver<RangeRequest>,
) -> Result<(), VfsError> {
    while let Some(request) = requests.recv().await {
        let result: Result<Vec<u8>, VfsError> = async {
            let reader = provider
                .read_range(&archive_path, request.offset, request.length)
                .await?;
            let mut limited = reader.take(request.length);
            let mut data = Vec::with_capacity(request.length as usize);
            limited.read_to_end(&mut data).await.map_err(|error| {
                VfsError::IoError(format!(
                    "ZIP range read failed at {}..{}: {error}",
                    request.offset,
                    request.offset.saturating_add(request.length)
                ))
            })?;
            if data.len() != request.length as usize {
                return Err(VfsError::IoError(format!(
                    "ZIP range read at offset {} returned {} bytes, expected {}",
                    request.offset,
                    data.len(),
                    request.length
                )));
            }
            Ok(data)
        }
        .await;

        match result {
            Ok(data) => {
                if request.response.send(Ok(data)).is_err() {
                    continue;
                }
            }
            Err(error) => {
                let message = error.to_string();
                let _ = request.response.send(Err(message));
                return Err(error);
            }
        }
    }
    Ok(())
}

enum ExtractCommand {
    Directory { rel_path: String },
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
    mut rx: mpsc::Receiver<ExtractCommand>,
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

fn send_command(tx: &mpsc::Sender<ExtractCommand>, command: ExtractCommand) -> Result<(), VfsError> {
    tx.blocking_send(command).map_err(|_| {
        VfsError::IoError("Archive extraction consumer stopped unexpectedly".into())
    })
}

fn stream_reader<R: Read>(
    reader: &mut R,
    tx: &mpsc::Sender<ExtractCommand>,
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

fn run_zip_worker<R: Read + Seek>(
    reader: R,
    tx: mpsc::Sender<ExtractCommand>,
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

async fn extract_zip_range_streaming(
    provider: &Arc<dyn FileSystem>,
    archive_path: &VfsPath,
    archive_size: u64,
    target_dir: &str,
    overwrite_mode: ArchiveOverwriteMode,
) -> Result<(usize, usize), VfsError> {
    let (request_tx, request_rx) = mpsc::channel::<RangeRequest>(1);
    let range_reader = BlockingRangeReader::new(request_tx, archive_size);
    let (command_tx, command_rx) = mpsc::channel(EXTRACT_STREAM_CHANNEL_CAPACITY);

    let worker = tokio::task::spawn_blocking(move || run_zip_worker(range_reader, command_tx));
    let range_server = serve_range_requests(
        Arc::clone(provider),
        archive_path.clone(),
        request_rx,
    );
    let consumer = consume_commands(
        Arc::clone(provider),
        &archive_path.connection_id,
        target_dir,
        overwrite_mode,
        command_rx,
    );

    let (range_result, consumer_result) = tokio::join!(range_server, consumer);
    let worker_result = worker
        .await
        .map_err(|error| VfsError::IoError(format!("ZIP range extraction task panicked: {error}")))?;

    match (consumer_result, worker_result, range_result) {
        (Err(consumer_error), _, _) => Err(consumer_error),
        (Ok(_), Err(worker_error), _) => Err(worker_error),
        (Ok(_), Ok(()), Err(range_error)) => Err(range_error),
        (Ok(counts), Ok(()), Ok(())) => Ok(counts),
    }
}

pub async fn extract_zip_adaptive(
    provider: &Arc<dyn FileSystem>,
    archive_path: &VfsPath,
    target_dir: &str,
    overwrite_mode: ArchiveOverwriteMode,
) -> Result<(usize, usize), VfsError> {
    let metadata = provider.stat(archive_path).await?;
    if metadata.kind != FileKind::File
        || metadata.size < ZIP_RANGE_MIN_ARCHIVE_SIZE
        || !provider.supports_efficient_range_read()
    {
        return extract_zip_streaming(provider, archive_path, target_dir, overwrite_mode).await;
    }

    extract_zip_range_streaming(
        provider,
        archive_path,
        metadata.size,
        target_dir,
        overwrite_mode,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::{
        BlockingRangeReader, RangeRequest, ZIP_RANGE_CACHE_CHUNKS, ZIP_RANGE_CHUNK_SIZE,
    };
    use std::io::{Read, Seek, SeekFrom};
    use std::sync::mpsc as std_mpsc;
    use tokio::sync::mpsc;

    #[test]
    fn range_reader_seeks_across_cached_chunks() {
        let total = ZIP_RANGE_CHUNK_SIZE * 3 + 257;
        let (tx, mut rx) = mpsc::channel::<RangeRequest>(1);
        let server = std::thread::spawn(move || {
            while let Some(request) = rx.blocking_recv() {
                let data = (0..request.length)
                    .map(|index| ((request.offset + index) % 251) as u8)
                    .collect();
                request.response.send(Ok(data)).unwrap();
            }
        });

        let mut reader = BlockingRangeReader::new(tx, total);
        reader.seek(SeekFrom::Start(ZIP_RANGE_CHUNK_SIZE - 3)).unwrap();
        let mut bytes = [0u8; 10];
        reader.read_exact(&mut bytes).unwrap();
        for (index, actual) in bytes.iter().enumerate() {
            let offset = ZIP_RANGE_CHUNK_SIZE - 3 + index as u64;
            assert_eq!(*actual, (offset % 251) as u8);
        }

        reader.seek(SeekFrom::End(-4)).unwrap();
        let mut tail = [0u8; 4];
        reader.read_exact(&mut tail).unwrap();
        for (index, actual) in tail.iter().enumerate() {
            let offset = total - 4 + index as u64;
            assert_eq!(*actual, (offset % 251) as u8);
        }
        drop(reader);
        server.join().unwrap();
    }

    #[test]
    fn range_reader_cache_is_strictly_bounded() {
        let total = ZIP_RANGE_CHUNK_SIZE * (ZIP_RANGE_CACHE_CHUNKS as u64 + 2);
        let (tx, mut rx) = mpsc::channel::<RangeRequest>(1);
        let server = std::thread::spawn(move || {
            while let Some(request) = rx.blocking_recv() {
                request
                    .response
                    .send(Ok(vec![0u8; request.length as usize]))
                    .unwrap();
            }
        });

        let mut reader = BlockingRangeReader::new(tx, total);
        for chunk in 0..(ZIP_RANGE_CACHE_CHUNKS + 2) {
            reader
                .seek(SeekFrom::Start(chunk as u64 * ZIP_RANGE_CHUNK_SIZE))
                .unwrap();
            let mut byte = [0u8; 1];
            reader.read_exact(&mut byte).unwrap();
            assert!(reader.cache.len() <= ZIP_RANGE_CACHE_CHUNKS);
        }
        assert_eq!(reader.cache.len(), ZIP_RANGE_CACHE_CHUNKS);
        drop(reader);
        server.join().unwrap();
    }

    #[test]
    fn range_reader_propagates_server_errors() {
        let (tx, mut rx) = mpsc::channel::<RangeRequest>(1);
        let server = std::thread::spawn(move || {
            let request = rx.blocking_recv().unwrap();
            request
                .response
                .send(Err("synthetic range failure".into()))
                .unwrap();
        });

        let mut reader = BlockingRangeReader::new(tx, ZIP_RANGE_CHUNK_SIZE);
        let mut byte = [0u8; 1];
        let error = reader.read(&mut byte).unwrap_err();
        assert!(error.to_string().contains("synthetic range failure"));
        server.join().unwrap();
    }

    #[test]
    fn range_response_channel_is_synchronous() {
        let (tx, rx) = std_mpsc::sync_channel::<Result<Vec<u8>, String>>(1);
        tx.send(Ok(vec![1, 2, 3])).unwrap();
        assert_eq!(rx.recv().unwrap().unwrap(), vec![1, 2, 3]);
    }
}
