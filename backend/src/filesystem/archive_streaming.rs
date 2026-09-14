use super::archive::compress_targz;
use crate::domain::{FileKind, VfsPath};
use crate::errors::{SecurityError, VfsError};
use crate::vfs::FileSystem;
use bytes::Bytes;
use flate2::write::GzEncoder;
use flate2::Compression;
use futures::StreamExt;
use std::io::{self, Write};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncReadExt, ReadBuf};
use tokio::sync::mpsc;
use uuid::Uuid;

const ARCHIVE_STREAM_CHUNK: usize = 64 * 1024;
const ARCHIVE_STREAM_CHANNEL_CAPACITY: usize = 8;
const MAX_ARCHIVE_FILES: usize = 50_000;

enum ArchiveWriteCommand {
    StartEntry { path: String, size: u64 },
    Data(Vec<u8>),
    EndEntry { written: u64 },
    Finish,
    Abort,
}

/// `std::io::Write` endpoint owned by the blocking compression worker.
/// Each write is capped to one archive stream chunk so channel capacity also bounds memory.
struct BlockingChannelWriter {
    tx: mpsc::Sender<Bytes>,
}

impl Write for BlockingChannelWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let len = buf.len().min(ARCHIVE_STREAM_CHUNK);
        self.tx
            .blocking_send(Bytes::copy_from_slice(&buf[..len]))
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "archive output reader closed"))?;
        Ok(len)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Async reader counterpart for `BlockingChannelWriter`.
/// The type is naturally `Unpin`, so it satisfies the VFS `AsyncReadBox` contract.
struct ChannelAsyncReader {
    rx: mpsc::Receiver<Bytes>,
    current: Option<Bytes>,
    offset: usize,
}

impl AsyncRead for ChannelAsyncReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        loop {
            if let Some(chunk) = self.current.take() {
                let remaining = &chunk[self.offset..];
                if !remaining.is_empty() && buf.remaining() > 0 {
                    let len = remaining.len().min(buf.remaining());
                    buf.put_slice(&remaining[..len]);
                    self.offset += len;
                    if self.offset < chunk.len() {
                        self.current = Some(chunk);
                    } else {
                        self.offset = 0;
                    }
                    return Poll::Ready(Ok(()));
                }
                self.offset = 0;
            }

            match Pin::new(&mut self.rx).poll_recv(cx) {
                Poll::Ready(Some(chunk)) => {
                    self.current = Some(chunk);
                    self.offset = 0;
                }
                Poll::Ready(None) => return Poll::Ready(Ok(())),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

fn output_pipe() -> (BlockingChannelWriter, ChannelAsyncReader) {
    let (tx, rx) = mpsc::channel(ARCHIVE_STREAM_CHANNEL_CAPACITY);
    (
        BlockingChannelWriter { tx },
        ChannelAsyncReader {
            rx,
            current: None,
            offset: 0,
        },
    )
}

async fn collect_archive_files(
    provider: &Arc<dyn FileSystem>,
    connection_id: &str,
    base_dir: &str,
    relative_paths: &[String],
) -> Result<Vec<(String, VfsPath)>, VfsError> {
    let mut files = Vec::new();

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
                    "Archive file count exceeded limit ({MAX_ARCHIVE_FILES})"
                ))
                .into());
            }
            files.push((rel.trim_start_matches('/').to_string(), full_vfs));
        } else if meta.kind == FileKind::Directory {
            let mut stack = vec![(rel.trim_start_matches('/').to_string(), full_vfs)];
            while let Some((parent_rel, parent_vfs)) = stack.pop() {
                let mut stream = provider.list_stream(&parent_vfs).await?;
                while let Some(entry) = stream.next().await {
                    let entry = entry?;
                    if files.len() >= MAX_ARCHIVE_FILES {
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
                        files.push((child_rel, child_vfs));
                    }
                }
            }
        }
    }

    Ok(files)
}

async fn send_reader_to_worker(
    mut reader: Box<dyn AsyncRead + Unpin + Send>,
    tx: &mpsc::Sender<ArchiveWriteCommand>,
) -> Result<u64, VfsError> {
    let mut buffer = vec![0u8; ARCHIVE_STREAM_CHUNK];
    let mut total = 0u64;
    loop {
        let n = reader
            .read(&mut buffer)
            .await
            .map_err(|error| VfsError::IoError(format!("Archive source read error: {error}")))?;
        if n == 0 {
            break;
        }
        total = total.saturating_add(n as u64);
        tx.send(ArchiveWriteCommand::Data(buffer[..n].to_vec()))
            .await
            .map_err(|_| VfsError::IoError("Tar compressor stopped unexpectedly".into()))?;
    }
    Ok(total)
}

fn staging_path(target: &VfsPath) -> Result<VfsPath, VfsError> {
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

async fn target_exists(
    provider: &Arc<dyn FileSystem>,
    target: &VfsPath,
) -> Result<bool, VfsError> {
    match provider.stat(target).await {
        Ok(_) => Ok(true),
        Err(VfsError::NotFound(_)) => Ok(false),
        Err(error) => Err(error),
    }
}

/// Stream TAR.GZ output directly into provider-side staging when atomic rename is available.
///
/// Existing destinations and providers without atomic rename deliberately fall back to the
/// established temp-file implementation so overwrite/commit semantics do not change.
pub async fn compress_targz_streaming(
    provider: &Arc<dyn FileSystem>,
    connection_id: &str,
    base_dir: &str,
    relative_paths: &[String],
    target_targz_path: &VfsPath,
) -> Result<(), VfsError> {
    if !provider.capabilities().atomic_rename || target_exists(provider, target_targz_path).await? {
        return compress_targz(
            provider,
            connection_id,
            base_dir,
            relative_paths,
            target_targz_path,
        )
        .await;
    }

    let files_to_pack =
        collect_archive_files(provider, connection_id, base_dir, relative_paths).await?;
    let staging = staging_path(target_targz_path)?;
    let (input_tx, mut input_rx) =
        mpsc::channel::<ArchiveWriteCommand>(ARCHIVE_STREAM_CHANNEL_CAPACITY);
    let (output_writer, output_reader) = output_pipe();

    let worker = tokio::task::spawn_blocking(move || -> Result<(), VfsError> {
        let mut encoder = GzEncoder::new(output_writer, Compression::default());
        let mut active_expected_size: Option<u64> = None;
        let mut active_written = 0u64;

        loop {
            match input_rx.blocking_recv() {
                Some(ArchiveWriteCommand::StartEntry { path, size }) => {
                    if active_expected_size.is_some() {
                        return Err(VfsError::IoError(
                            "Tar entry started before previous entry completed".into(),
                        ));
                    }
                    let mut header = tar::Header::new_gnu();
                    header
                        .set_path(&path)
                        .map_err(|error| VfsError::IoError(format!("Tar path error: {error}")))?;
                    header.set_size(size);
                    header.set_mode(0o644);
                    header.set_cksum();
                    encoder.write_all(header.as_bytes()).map_err(|error| {
                        VfsError::IoError(format!("Tar header write error: {error}"))
                    })?;
                    active_expected_size = Some(size);
                    active_written = 0;
                }
                Some(ArchiveWriteCommand::Data(data)) => {
                    if active_expected_size.is_none() {
                        return Err(VfsError::IoError("Tar data received without entry".into()));
                    }
                    encoder.write_all(&data).map_err(|error| {
                        VfsError::IoError(format!("Tar content write error: {error}"))
                    })?;
                    active_written = active_written.saturating_add(data.len() as u64);
                }
                Some(ArchiveWriteCommand::EndEntry { written }) => {
                    let expected_size = active_expected_size.take().ok_or_else(|| {
                        VfsError::IoError("Tar entry ended without active entry".into())
                    })?;
                    if written != active_written || written != expected_size {
                        return Err(VfsError::IoError(format!(
                            "Tar source size changed while archiving (expected {expected_size}, streamed {written})"
                        )));
                    }
                    let padding = (512 - (written % 512)) % 512;
                    if padding > 0 {
                        encoder
                            .write_all(&[0u8; 512][..padding as usize])
                            .map_err(|error| {
                                VfsError::IoError(format!("Tar padding write error: {error}"))
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
                    encoder.write_all(&[0u8; 1024]).map_err(|error| {
                        VfsError::IoError(format!("Tar trailer write error: {error}"))
                    })?;
                    encoder
                        .finish()
                        .map_err(|error| VfsError::IoError(format!("Gzip finish error: {error}")))?;
                    return Ok(());
                }
                Some(ArchiveWriteCommand::Abort) => return Ok(()),
                None => {
                    return Err(VfsError::IoError(
                        "Tar producer stopped before archive finalization".into(),
                    ));
                }
            }
        }
    });

    let producer = async {
        for (rel_path, full_vfs) in files_to_pack {
            let meta = provider.stat(&full_vfs).await?;
            input_tx
                .send(ArchiveWriteCommand::StartEntry {
                    path: rel_path,
                    size: meta.size,
                })
                .await
                .map_err(|_| VfsError::IoError("Tar compressor stopped unexpectedly".into()))?;
            let reader = provider.read_stream(&full_vfs).await?;
            let written = send_reader_to_worker(reader, &input_tx).await?;
            input_tx
                .send(ArchiveWriteCommand::EndEntry { written })
                .await
                .map_err(|_| VfsError::IoError("Tar compressor stopped unexpectedly".into()))?;
        }
        input_tx
            .send(ArchiveWriteCommand::Finish)
            .await
            .map_err(|_| VfsError::IoError("Tar compressor stopped unexpectedly".into()))?;
        Ok::<(), VfsError>(())
    };

    let upload = provider.write_stream(&staging, Box::new(output_reader));
    let (producer_result, upload_result) = tokio::join!(producer, upload);

    if producer_result.is_err() {
        let _ = input_tx.send(ArchiveWriteCommand::Abort).await;
    }
    drop(input_tx);

    let worker_result = worker
        .await
        .map_err(|error| VfsError::IoError(format!("Tar compressor task panicked: {error}")))?;

    let operation_result = match (upload_result, worker_result, producer_result) {
        (Err(error), _, _) => Err(error),
        (Ok(()), Err(error), _) => Err(error),
        (Ok(()), Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(()), Ok(())) => provider.rename(&staging, target_targz_path).await,
    };

    if operation_result.is_err() {
        let _ = provider.delete(&staging).await;
    }

    operation_result
}

#[cfg(test)]
mod tests {
    use super::{output_pipe, ARCHIVE_STREAM_CHUNK};
    use std::io::Write;
    use tokio::io::AsyncReadExt;

    #[tokio::test]
    async fn blocking_output_pipe_preserves_bytes_and_eof() {
        let (mut writer, mut reader) = output_pipe();
        let payload = vec![0x5au8; ARCHIVE_STREAM_CHUNK * 2 + 137];
        let expected = payload.clone();

        let worker = tokio::task::spawn_blocking(move || {
            writer.write_all(&payload).unwrap();
            writer.flush().unwrap();
        });

        let mut actual = Vec::new();
        reader.read_to_end(&mut actual).await.unwrap();
        worker.await.unwrap();
        assert_eq!(actual, expected);
    }
}
