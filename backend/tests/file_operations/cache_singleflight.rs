use backend::{
    domain::{FileKind, FileMetadata},
    services::MetadataCache,
};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::Notify;

fn metadata() -> FileMetadata {
    FileMetadata {
        name: "shared-file.txt".into(),
        path: "/shared-file.txt".into(),
        size: 4096,
        kind: FileKind::File,
        modified_at: None,
        created_at: None,
        mime_type: None,
        etag: "etag-123".into(),
        permissions: None,
        is_readonly: false,
        is_hidden: false,
        symlink_target: None,
    }
}

#[tokio::test]
async fn concurrent_metadata_misses_coalesce_into_one_successful_fetch() {
    let cache = MetadataCache::new(Duration::from_secs(5));
    let fetch_count = Arc::new(AtomicUsize::new(0));
    let release = Arc::new(Notify::new());
    let mut tasks = Vec::new();

    for _ in 0..10 {
        let cache = cache.clone();
        let fetch_count = fetch_count.clone();
        let release = release.clone();
        tasks.push(tokio::spawn(async move {
            cache
                .get_or_fetch("local", "/shared-file.txt", || async move {
                    fetch_count.fetch_add(1, Ordering::SeqCst);
                    release.notified().await;
                    Ok(metadata())
                })
                .await
        }));
    }

    for _ in 0..100 {
        if fetch_count.load(Ordering::SeqCst) == 1 {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(fetch_count.load(Ordering::SeqCst), 1);
    release.notify_one();

    for task in tasks {
        let value = task.await.unwrap().unwrap();
        assert_eq!(value.size, 4096);
    }
    assert_eq!(fetch_count.load(Ordering::SeqCst), 1);
}
