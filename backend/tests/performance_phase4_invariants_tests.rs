use std::{fs, path::PathBuf};

fn source(path: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(root.join(path))
        .unwrap_or_else(|error| panic!("failed to read Phase 4 guard source '{path}': {error}"))
}

fn section<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start_index = source
        .find(start)
        .unwrap_or_else(|| panic!("Phase 4 guard start marker missing: {start}"));
    let rest = &source[start_index..];
    let end_index = rest
        .find(end)
        .unwrap_or_else(|| panic!("Phase 4 guard end marker missing: {end}"));
    &rest[..end_index]
}

#[test]
fn phase4_inline_uploads_share_transfer_resource_budget() {
    let uploads = source("src/infrastructure/uploads.rs");
    let execute = section(
        &uploads,
        "async fn execute_inline(",
        "async fn complete_inline_job",
    );

    assert!(
        uploads.contains("resource_budget: Arc<ResourceBudget>"),
        "Phase 4 regression: inline upload adapter must retain the shared ResourceBudget"
    );
    assert!(
        execute.contains("ResourceClass::TransferMixed"),
        "Phase 4 regression: HTTP -> local uploads must consume local + network + transfer capacity"
    );
    assert!(
        execute.contains("ResourceClass::TransferNetwork"),
        "Phase 4 regression: HTTP -> remote uploads must consume network + transfer capacity"
    );
    assert!(
        execute.contains("self.resource_budget.acquire(resource_class)"),
        "Phase 4 regression: inline upload execution must acquire the shared admission budget"
    );
    assert!(
        execute.contains("cancel_token.cancelled()"),
        "Phase 4 regression: waiting for resource capacity must remain cancellation-aware"
    );
}

#[test]
fn phase4_bootstrap_wires_the_same_budget_into_queued_and_inline_transfers() {
    let bootstrap = source("src/bootstrap.rs");

    assert!(
        bootstrap.contains(
            "TransferManager::new(\n        registry.providers_map(),\n        db.clone(),\n        config.limits.max_concurrent_transfers,\n        resource_budget.clone(),"
        ),
        "Phase 4 regression: queued transfers must use the system ResourceBudget"
    );
    assert!(
        bootstrap.contains(
            "TransferUploadExecution::new(\n        transfer_manager.clone(),\n        resource_budget.clone(),"
        ),
        "Phase 4 regression: inline uploads must receive the exact shared ResourceBudget"
    );
}

#[test]
fn phase4_targz_compression_streams_traversal_without_collecting_all_files() {
    let archive = source("src/filesystem/archive_compress_stream.rs");
    let compressor = section(
        &archive,
        "pub async fn compress_targz_streaming(",
        "#[cfg(test)]",
    );

    assert!(
        archive.contains("async fn stream_archive_files("),
        "Phase 4 regression: TAR.GZ compression must retain incremental provider traversal"
    );
    assert!(
        archive.contains("stream_archive_file(provider, child_rel, &child_vfs, tx).await?;"),
        "Phase 4 regression: discovered files must stream directly into the compressor pipeline"
    );
    assert!(
        !archive.contains("collect_archive_files"),
        "Phase 4 regression: TAR.GZ compression must not restore the upfront file metadata Vec"
    );
    assert!(
        !compressor.contains("files_to_pack"),
        "Phase 4 regression: compression must start before the complete traversal is materialized"
    );
    assert!(
        compressor.contains("ARCHIVE_STREAM_CHANNEL_CAPACITY"),
        "Phase 4 regression: TAR.GZ compressor input must remain bounded"
    );
}
