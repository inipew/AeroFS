use std::path::Path;
use std::time::{Duration, SystemTime};

/// Scans local root recursively and deletes orphan staging files (*.aerofs.part, .*.aerofs-part-*)
/// older than max_age (e.g. 24 hours). Returns count of deleted files.
pub async fn cleanup_stale_staging_files(root: &Path, max_age: Duration) -> usize {
    if !root.exists() {
        return 0;
    }

    let now = SystemTime::now();
    let mut deleted_count = 0;
    let mut dirs_to_visit = vec![root.to_path_buf()];

    while let Some(current_dir) = dirs_to_visit.pop() {
        let mut entries = match tokio::fs::read_dir(&current_dir).await {
            Ok(e) => e,
            Err(_) => continue,
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            let file_type = match entry.file_type().await {
                Ok(ft) => ft,
                Err(_) => continue,
            };

            if file_type.is_dir() {
                dirs_to_visit.push(path);
            } else if file_type.is_file() {
                let file_name = entry.file_name().to_string_lossy().to_string();
                if file_name.ends_with(".aerofs.part") || file_name.contains(".aerofs-part-") {
                    if let Ok(meta) = entry.metadata().await {
                        if let Ok(mtime) = meta.modified() {
                            if let Ok(age) = now.duration_since(mtime) {
                                if age >= max_age && tokio::fs::remove_file(&path).await.is_ok() {
                                    tracing::info!(
                                        "Cleaned up stale orphan staging file: {:?} (age: {:?})",
                                        path,
                                        age
                                    );
                                    deleted_count += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    deleted_count
}
