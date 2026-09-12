use std::fs;
use std::path::PathBuf;

fn source(path: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(root.join(path)).expect("source file should be readable")
}

#[test]
fn transfer_handlers_only_use_application_transfer_boundary() {
    let api = source("src/api/transfers.rs");
    assert!(api.contains("State<TransferState>"));
    assert!(api.contains("state.use_cases"));
    assert!(!api.contains("State<AppState>"));
    assert!(!api.contains("TransferService"));
    assert!(!api.contains("state.transfer_manager"));
    assert!(!api.contains("check_permission("));
}

#[test]
fn file_handlers_do_not_reconstruct_compatibility_facade() {
    let api = source("src/api/files.rs");
    assert!(!api.contains("FileApplicationService::from_state"));
    assert!(!api.contains("FileApplicationService::new"));
    assert!(api.contains(".write_file"));
    assert!(api.contains(".presign_download"));
    assert!(api.contains(".chmod_entry"));
}

#[test]
fn file_copy_orchestration_lives_outside_http_layer() {
    let api = source("src/api/files.rs");
    let start = api.find("pub async fn copy_entry").expect("copy handler");
    let tail = &api[start..];
    let end = tail
        .find("/// Admit a streaming upload")
        .expect("next handler marker");
    let handler = &tail[..end];

    assert!(handler.contains(".copy_entry"));
    assert!(!handler.contains("provider.copy"));
    assert!(!handler.contains("record_audit_log"));
    assert!(!handler.contains("metadata_cache"));
    assert!(!handler.contains("event_journal"));
}
