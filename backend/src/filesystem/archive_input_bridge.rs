use crate::errors::VfsError;
use crate::vfs::traits::AsyncReadBox;
use bytes::Bytes;
use std::io::{self, Read};
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;

pub(crate) const ARCHIVE_INPUT_CHUNK: usize = 64 * 1024;
pub(crate) const ARCHIVE_INPUT_CHANNEL_CAPACITY: usize = 8;

type InputChunk = Result<Bytes, String>;

pub(crate) struct BlockingChannelReader {
    rx: mpsc::Receiver<InputChunk>,
    current: Option<Bytes>,
    offset: usize,
}

impl Read for BlockingChannelReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        loop {
            if let Some(chunk) = self.current.take() {
                let remaining = &chunk[self.offset..];
                if !remaining.is_empty() {
                    let len = remaining.len().min(buf.len());
                    buf[..len].copy_from_slice(&remaining[..len]);
                    self.offset += len;
                    if self.offset < chunk.len() {
                        self.current = Some(chunk);
                    } else {
                        self.offset = 0;
                    }
                    return Ok(len);
                }
                self.offset = 0;
            }

            match self.rx.blocking_recv() {
                Some(Ok(chunk)) => {
                    self.current = Some(chunk);
                    self.offset = 0;
                }
                Some(Err(message)) => return Err(io::Error::other(message)),
                None => return Ok(0),
            }
        }
    }
}

pub(crate) fn input_pipe() -> (mpsc::Sender<InputChunk>, BlockingChannelReader) {
    let (tx, rx) = mpsc::channel(ARCHIVE_INPUT_CHANNEL_CAPACITY);
    (
        tx,
        BlockingChannelReader {
            rx,
            current: None,
            offset: 0,
        },
    )
}

pub(crate) async fn pump_async_reader(
    mut reader: AsyncReadBox,
    tx: mpsc::Sender<InputChunk>,
) -> Result<(), VfsError> {
    let mut buffer = vec![0u8; ARCHIVE_INPUT_CHUNK];
    loop {
        let read = match reader.read(&mut buffer).await {
            Ok(read) => read,
            Err(error) => {
                let message = format!("Archive source read error: {error}");
                let _ = tx.send(Err(message.clone())).await;
                return Err(VfsError::IoError(message));
            }
        };

        if read == 0 {
            return Ok(());
        }

        tx.send(Ok(Bytes::copy_from_slice(&buffer[..read])))
            .await
            .map_err(|_| {
                VfsError::IoError("Archive decompressor stopped before input completed".into())
            })?;
    }
}

#[cfg(test)]
mod tests {
    use super::{input_pipe, ARCHIVE_INPUT_CHUNK};
    use bytes::Bytes;
    use std::io::Read;

    #[test]
    fn blocking_reader_preserves_chunk_boundaries_and_eof() {
        let (tx, mut reader) = input_pipe();
        tx.blocking_send(Ok(Bytes::from(vec![0x11; ARCHIVE_INPUT_CHUNK])))
            .unwrap();
        tx.blocking_send(Ok(Bytes::from_static(b"tail"))).unwrap();
        drop(tx);

        let mut actual = Vec::new();
        reader.read_to_end(&mut actual).unwrap();
        assert_eq!(actual.len(), ARCHIVE_INPUT_CHUNK + 4);
        assert!(actual[..ARCHIVE_INPUT_CHUNK].iter().all(|byte| *byte == 0x11));
        assert_eq!(&actual[ARCHIVE_INPUT_CHUNK..], b"tail");
    }

    #[test]
    fn blocking_reader_propagates_source_error() {
        let (tx, mut reader) = input_pipe();
        tx.blocking_send(Err("source failed".into())).unwrap();
        drop(tx);

        let mut byte = [0u8; 1];
        let error = reader.read(&mut byte).unwrap_err();
        assert_eq!(error.to_string(), "source failed");
    }

    #[test]
    fn zero_length_read_does_not_consume_buffered_data() {
        let (tx, mut reader) = input_pipe();
        tx.blocking_send(Ok(Bytes::from_static(b"abc"))).unwrap();
        drop(tx);

        assert_eq!(reader.read(&mut []).unwrap(), 0);
        let mut actual = [0u8; 3];
        assert_eq!(reader.read(&mut actual).unwrap(), 3);
        assert_eq!(&actual, b"abc");
    }
}
