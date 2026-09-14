use backend::{
    domain::{Capabilities, VfsPath},
    filesystem::{
        archive::ArchiveOverwriteMode,
        archive_stream::{extract_targz_streaming, extract_zip_streaming},
    },
    vfs::FileSystem,
};
use flate2::{write::GzEncoder, Compression};
use std::io::{Cursor, Write};
use std::sync::Arc;
use zip::write::SimpleFileOptions;

use crate::support::MemoryFileSystem;

fn archive_capabilities() -> Capabilities {
    Capabilities {
        read: true,
        write: true,
        create_file: true,
        create_dir: true,
        delete: true,
        rename: true,
        copy: true,
        permissions: true,
        ..Default::default()
    }
}

fn zip_bytes(path: &str, content: &[u8]) -> Vec<u8> {
    let cursor = Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(cursor);
    writer
        .start_file(
            path,
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
        )
        .unwrap();
    writer.write_all(content).unwrap();
    writer.finish().unwrap().into_inner()
}

fn targz_bytes(path: &str, content: &[u8]) -> Vec<u8> {
    let encoder = GzEncoder::new(Vec::new(), Compression::default());
    let mut builder = tar::Builder::new(encoder);
    let mut header = tar::Header::new_gnu();
    header.set_size(content.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    builder.append_data(&mut header, path, content).unwrap();
    let encoder = builder.into_inner().unwrap();
    encoder.finish().unwrap()
}

#[tokio::test]
async fn zip_and_targz_stream_nested_files_to_provider() {
    let zip_fs = Arc::new(MemoryFileSystem::new(archive_capabilities()));
    zip_fs.insert_file("/input.zip", zip_bytes("nested/hello.txt", b"hello zip"));
    let zip_provider: Arc<dyn FileSystem> = zip_fs.clone();
    let archive = VfsPath::new("memory", "/input.zip").unwrap();
    let result = extract_zip_streaming(
        &zip_provider,
        &archive,
        "/output",
        ArchiveOverwriteMode::Overwrite,
    )
    .await
    .unwrap();
    assert_eq!(result, (1, 0));
    assert_eq!(
        zip_fs.bytes("/output/nested/hello.txt").as_deref(),
        Some(b"hello zip".as_slice())
    );

    let tar_fs = Arc::new(MemoryFileSystem::new(archive_capabilities()));
    tar_fs.insert_file(
        "/input.tar.gz",
        targz_bytes("nested/hello.txt", b"hello targz"),
    );
    let tar_provider: Arc<dyn FileSystem> = tar_fs.clone();
    let archive = VfsPath::new("memory", "/input.tar.gz").unwrap();
    let result = extract_targz_streaming(
        &tar_provider,
        &archive,
        "/output",
        ArchiveOverwriteMode::Overwrite,
    )
    .await
    .unwrap();
    assert_eq!(result, (1, 0));
    assert_eq!(
        tar_fs.bytes("/output/nested/hello.txt").as_deref(),
        Some(b"hello targz".as_slice())
    );
}

#[tokio::test]
async fn overwrite_modes_preserve_skip_and_keep_both_semantics() {
    let fs = Arc::new(MemoryFileSystem::new(archive_capabilities()));
    fs.insert_file("/input.zip", zip_bytes("hello.txt", b"replacement"));
    fs.insert_dir("/output");
    fs.insert_file("/output/hello.txt", b"original".to_vec());
    let provider: Arc<dyn FileSystem> = fs.clone();
    let archive = VfsPath::new("memory", "/input.zip").unwrap();

    let skipped = extract_zip_streaming(
        &provider,
        &archive,
        "/output",
        ArchiveOverwriteMode::Skip,
    )
    .await
    .unwrap();
    assert_eq!(skipped, (0, 1));
    assert_eq!(fs.bytes("/output/hello.txt").as_deref(), Some(b"original".as_slice()));

    let kept = extract_zip_streaming(
        &provider,
        &archive,
        "/output",
        ArchiveOverwriteMode::KeepBoth,
    )
    .await
    .unwrap();
    assert_eq!(kept, (1, 0));
    assert_eq!(
        fs.bytes("/output/hello (1).txt").as_deref(),
        Some(b"replacement".as_slice())
    );
}

#[tokio::test]
async fn archive_permission_failure_surfaces_partial_commit_recovery_semantics() {
    let fs = Arc::new(MemoryFileSystem::new(archive_capabilities()));
    fs.insert_file("/input.zip", zip_bytes("hello.txt", b"committed content"));
    fs.insert_dir("/output");
    fs.set_permissions(&VfsPath::new("memory", "/output").unwrap(), "0750")
        .await
        .unwrap();
    fs.fail_permissions("chmod denied");

    let provider: Arc<dyn FileSystem> = fs.clone();
    let error = extract_zip_streaming(
        &provider,
        &VfsPath::new("memory", "/input.zip").unwrap(),
        "/output",
        ArchiveOverwriteMode::Overwrite,
    )
    .await
    .expect_err("chmod failure after provider write must be visible");

    assert!(error
        .to_string()
        .contains("Filesystem mutation committed; recovery required"));
    assert_eq!(
        fs.bytes("/output/hello.txt").as_deref(),
        Some(b"committed content".as_slice()),
        "archive payload was committed before chmod failed"
    );
}
