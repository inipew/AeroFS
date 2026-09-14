use backend::{
    domain::{Capabilities, VfsPath},
    errors::VfsError,
    vfs::{
        registry::ProviderRegistry,
        runtime::{ProviderLoader, StorageRuntime},
        FileSystem,
    },
};
use futures::StreamExt;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::io::AsyncReadExt;

use crate::support::MemoryFileSystem;

fn capabilities() -> Capabilities {
    Capabilities {
        stat: true,
        read: true,
        write: true,
        create_file: true,
        create_dir: true,
        ..Default::default()
    }
}

#[tokio::test]
async fn active_lease_blocks_reclamation_then_remote_provider_rehydrates_lazily() {
    let registry = ProviderRegistry::new();
    let initial = Arc::new(MemoryFileSystem::new(capabilities()));
    initial.insert_file("/before.txt", b"before".to_vec());
    let load_count = Arc::new(AtomicUsize::new(0));
    let load_count_for_loader = load_count.clone();
    let loader: ProviderLoader = Arc::new(move || {
        load_count_for_loader.fetch_add(1, Ordering::SeqCst);
        let restored = Arc::new(MemoryFileSystem::new(capabilities()));
        restored.insert_file("/after.txt", b"after".to_vec());
        Ok(restored as Arc<dyn FileSystem>)
    });

    registry
        .register_reclaimable("remote-a".into(), initial, loader)
        .await;
    let runtime = registry.get_runtime("remote-a").await.unwrap();
    let provider = registry.get("remote-a").await.unwrap();

    let lease = runtime.acquire_lease().await;
    assert_eq!(runtime.active_leases_count(), 1);
    assert!(registry
        .reclaim_idle_connections(Duration::ZERO)
        .await
        .is_empty());

    drop(lease);
    assert_eq!(runtime.active_leases_count(), 0);
    assert_eq!(
        registry.reclaim_idle_connections(Duration::ZERO).await,
        vec!["remote-a".to_string()]
    );
    assert_eq!(load_count.load(Ordering::SeqCst), 0);

    let metadata = provider
        .stat(&VfsPath::new("remote-a", "/after.txt").unwrap())
        .await
        .unwrap();
    assert_eq!(metadata.size, 5);
    assert_eq!(load_count.load(Ordering::SeqCst), 1);
    assert_eq!(runtime.active_leases_count(), 0);
}

#[tokio::test]
async fn streaming_reader_and_directory_stream_hold_lease_until_consumer_drops_them() {
    let registry = ProviderRegistry::new();
    let memory = Arc::new(MemoryFileSystem::new(capabilities()));
    memory.insert_file("/payload.bin", b"payload".to_vec());
    memory.insert_file("/listed.txt", b"listed".to_vec());
    let loader_memory = memory.clone();
    let loader: ProviderLoader = Arc::new(move || Ok(loader_memory.clone() as Arc<dyn FileSystem>));
    registry
        .register_reclaimable("remote-stream".into(), memory, loader)
        .await;

    let runtime = registry.get_runtime("remote-stream").await.unwrap();
    let provider = registry.get("remote-stream").await.unwrap();

    let mut reader = provider
        .read_stream(&VfsPath::new("remote-stream", "/payload.bin").unwrap())
        .await
        .unwrap();
    assert_eq!(runtime.active_leases_count(), 1);
    let mut prefix = [0u8; 3];
    reader.read_exact(&mut prefix).await.unwrap();
    assert_eq!(&prefix, b"pay");
    assert_eq!(runtime.active_leases_count(), 1);
    drop(reader);
    assert_eq!(runtime.active_leases_count(), 0);

    let mut entries = provider
        .list_stream(&VfsPath::root("remote-stream"))
        .await
        .unwrap();
    assert_eq!(runtime.active_leases_count(), 1);
    assert!(entries.next().await.unwrap().is_ok());
    assert_eq!(runtime.active_leases_count(), 1);
    drop(entries);
    assert_eq!(runtime.active_leases_count(), 0);
}

#[tokio::test]
async fn local_registry_entry_is_never_selected_for_idle_reclamation() {
    let registry = ProviderRegistry::new();
    let local = Arc::new(MemoryFileSystem::new(capabilities()));
    let loader_local = local.clone();
    let loader: ProviderLoader = Arc::new(move || Ok(loader_local.clone() as Arc<dyn FileSystem>));
    registry
        .register_reclaimable("local".into(), local, loader)
        .await;

    assert!(registry
        .reclaim_idle_connections(Duration::ZERO)
        .await
        .is_empty());
    assert!(registry.contains("local").await);
}

#[tokio::test]
async fn failed_lazy_rehydrate_surfaces_connection_error_without_installing_provider() {
    let initial: Arc<dyn FileSystem> = Arc::new(MemoryFileSystem::new(capabilities()));
    let loader: ProviderLoader = Arc::new(|| Err(anyhow::anyhow!("credential refresh failed")));
    let runtime = Arc::new(StorageRuntime::new_reclaimable(
        "remote-fail",
        initial,
        4,
        loader,
    ));
    let provider = backend::vfs::runtime::BudgetedFileSystem::new(runtime.clone());

    assert!(runtime.reclaim_if_idle(Duration::ZERO).await);
    let error = provider
        .stat(&VfsPath::root("remote-fail"))
        .await
        .expect_err("rehydration failure must reach the caller");
    assert!(matches!(error, VfsError::ConnectionError(message) if message.contains("credential refresh failed")));
    assert_eq!(runtime.active_leases_count(), 0);
}
