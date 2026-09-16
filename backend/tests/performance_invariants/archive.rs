use super::performance_support::{section, source};

#[test]
fn targz_compression_streams_traversal_without_collecting_all_files() {
    let archive = source("src/filesystem/archive_compress_stream.rs");
    let compressor = section(
        &archive,
        "pub async fn compress_targz_streaming(",
        "#[cfg(test)]",
    );

    assert!(
        archive.contains("async fn stream_archive_files("),
        "performance regression: TAR.GZ compression must retain incremental provider traversal"
    );
    assert!(
        archive.contains("stream_archive_file(provider, child_rel, &child_vfs, tx).await?;"),
        "performance regression: discovered files must stream directly into the compressor pipeline"
    );
    assert!(
        !archive.contains("collect_archive_files"),
        "performance regression: TAR.GZ compression must not restore the upfront file metadata Vec"
    );
    assert!(
        !compressor.contains("files_to_pack"),
        "performance regression: compression must start before the complete traversal is materialized"
    );
    assert!(
        compressor.contains("ARCHIVE_STREAM_CHANNEL_CAPACITY"),
        "performance regression: TAR.GZ compressor input must remain bounded"
    );
}

#[test]
fn targz_existing_local_target_uses_safe_atomic_replace() {
    let archive = source("src/filesystem/archive_compress_stream.rs");
    let compressor = section(
        &archive,
        "pub async fn compress_targz_streaming(",
        "#[cfg(test)]",
    );

    assert!(
        archive.contains("fn can_replace_atomically(provider: &Arc<dyn FileSystem>) -> bool"),
        "performance regression: archive commit policy must keep an explicit atomic-replace predicate"
    );
    assert!(
        archive.contains("provider.is_local() && capabilities.atomic_rename"),
        "performance regression: existing-target streaming must remain conservative and local-only"
    );
    assert!(
        compressor.contains("(exists && !can_replace_atomically(provider))"),
        "performance regression: unsafe existing-target providers must retain the fallback path"
    );
    assert!(
        compressor.contains("provider.rename(&staging, target_targz_path).await"),
        "performance regression: safe streaming commits must promote provider-side staging atomically"
    );
}

#[test]
fn zip_compression_streams_traversal_and_stages_commits() {
    let zip = source("src/filesystem/archive_zip_stream.rs");
    let service = source("src/services/archive_service.rs");

    assert!(
        zip.contains("async fn stream_archive_files("),
        "performance regression: ZIP compression must not collect the full directory tree before work starts"
    );
    assert!(
        !zip.contains("collect_archive_files"),
        "performance regression: ZIP compression must not restore the upfront file metadata Vec"
    );
    assert!(
        zip.contains("mpsc::channel::<ArchiveWriteCommand>(ARCHIVE_STREAM_CHANNEL_CAPACITY)"),
        "performance regression: ZIP producer-to-compressor handoff must remain bounded"
    );
    assert!(
        zip.contains("let can_stage_commit = capabilities.atomic_rename"),
        "performance regression: ZIP provider-side staging must stay gated by rename semantics"
    );
    assert!(
        zip.contains("provider.rename(&staging, target_zip_path).await"),
        "performance regression: safe ZIP commits must promote provider-side staging"
    );
    assert!(
        zip.contains("tempfile::NamedTempFile::new()"),
        "performance regression: ZIP must retain a seekable local file for ZipWriter central-directory finalization"
    );
    assert!(
        service.contains("compress_zip_streaming("),
        "performance regression: ArchiveService must continue using the hardened ZIP path"
    );
}

#[test]
fn large_remote_zip_extraction_uses_bounded_range_seek() {
    let zip = source("src/filesystem/archive_zip_range.rs");
    let traits = source("src/vfs/traits.rs");
    let service = source("src/services/archive_service.rs");

    assert!(
        zip.contains("ZIP_RANGE_CHUNK_SIZE: u64 = 2 * 1024 * 1024"),
        "performance regression: ZIP range requests must stay coarse enough to avoid tiny-read amplification"
    );
    assert!(
        zip.contains("ZIP_RANGE_CACHE_CHUNKS: usize = 4"),
        "performance regression: ZIP random-access cache must remain strictly bounded"
    );
    assert!(
        zip.contains("ZIP_RANGE_MIN_ARCHIVE_SIZE: u64 = 16 * 1024 * 1024"),
        "performance regression: small ZIP files should retain the cheaper sequential temp-file path"
    );
    assert!(
        zip.contains(".read_range(&archive_path, request.offset, request.length)"),
        "performance regression: large remote ZIP extraction must use provider range reads"
    );
    assert!(
        zip.contains("return extract_zip_streaming(provider, archive_path, target_dir, overwrite_mode).await;"),
        "performance regression: providers without efficient native ranges must retain the safe fallback"
    );
    assert!(
        traits.contains("fn supports_efficient_range_read(&self) -> bool"),
        "performance regression: seek-heavy ZIP reads need an explicit efficient-range capability gate"
    );
    assert!(
        traits.contains("capabilities.server_side_copy")
            && traits.contains("capabilities.native_checksum"),
        "performance regression: default efficient-range profile must remain conservative for S3-like providers"
    );
    assert!(
        service.contains("extract_zip_adaptive("),
        "performance regression: ArchiveService must route ZIP extraction through the adaptive path"
    );
}

#[test]
fn archive_extractors_share_one_consumer_and_output_policy() {
    let core = source("src/filesystem/archive_extract_core.rs");
    let stream = source("src/filesystem/archive_stream.rs");
    let range = source("src/filesystem/archive_zip_range.rs");

    assert!(
        core.contains("pub(crate) enum ExtractCommand")
            && core.contains("pub(crate) async fn consume_commands("),
        "performance regression: extraction command protocol and consumer must stay centralized"
    );
    assert!(
        core.contains("ArchiveOverwriteMode::Skip")
            && core.contains("ArchiveOverwriteMode::Overwrite")
            && core.contains("ArchiveOverwriteMode::KeepBoth"),
        "performance regression: overwrite policy must remain owned by the shared extraction core"
    );
    assert!(
        core.contains("resolve_destination_permissions_strict")
            && core.contains("write_stream(&destination_for_write, Box::new(reader))"),
        "performance regression: permission inheritance and provider output streaming must remain centralized"
    );
    assert!(
        stream.contains("consume_commands(") && stream.contains("request_file_stream("),
        "performance regression: temp ZIP and TAR.GZ extraction must use the shared core"
    );
    assert!(
        range.contains("run_zip_reader(range_reader, command_tx)")
            && range.contains("consume_commands("),
        "performance regression: range-backed ZIP extraction must reuse the same parser and consumer"
    );
    assert!(
        !range.contains("enum ExtractCommand")
            && !range.contains("struct ActiveFile")
            && !range.contains("resolve_file_destination"),
        "performance regression: range-backed ZIP must not reintroduce a second extraction policy implementation"
    );
}
