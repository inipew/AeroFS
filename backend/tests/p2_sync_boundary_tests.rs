use std::fs;

fn source(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"))
}

fn compact(src: &str) -> String {
    src.chars().filter(|c| !c.is_whitespace()).collect()
}

#[test]
fn sync_service_depends_on_control_port_not_runtime_manager() {
    let src = source("src/services/sync_service.rs");
    let compact = compact(&src);

    assert!(compact.contains("Arc<dynSyncControl>"));
    assert!(!src.contains("SyncManager"));
    assert!(!src.contains("crate::infrastructure"));
}

#[test]
fn durable_sync_manager_remains_the_runtime_recovery_owner() {
    let bootstrap = compact(&source("src/bootstrap.rs"));
    let adapter = compact(&source("src/infrastructure/sync.rs"));

    assert!(bootstrap.contains("let_sync_manager=Arc::new(SyncManager::new(") || bootstrap.contains("letsync_manager=Arc::new(SyncManager::new("));
    assert!(bootstrap.contains("SyncEventSubscriber::spawn("));
    assert!(bootstrap.contains("SyncService::new(file_authorization.clone(),sync_manager"));
    assert!(adapter.contains("implSyncControlforSyncManager"));
}
