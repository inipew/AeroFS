use std::path::Path;
use std::time::{Duration, SystemTime};

/// Scans local root recursively and deletes orphan staging files (*.aerofs.part, .*.aerofs-part-*)
/// older than max_age (e.g. 24 hours). Returns count of deleted files.
///
/// Filesystem errors are deliberately surfaced rather than skipped so the runtime
/// supervisor can distinguish a healthy cleanup pass from a persistently broken
/// storage root. A root that does not exist is treated as an empty cleanup pass.
pub async fn cleanup_stale_staging_files(
    root: &Path,
    max_age: Duration,
) -> std::io::Result<usize> {
    if !root.exists() {
        return Ok(0);
    }

    let now = SystemTime::now();
    let mut deleted_count = 0;
    let mut dirs_to_visit = vec![root.to_path_buf()];

    while let Some(current_dir) = dirs_to_visit.pop() {
        let mut entries = tokio::fs::read_dir(&current_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let file_type = entry.file_type().await?;

            if file_type.is_dir() {
                dirs_to_visit.push(path);
            } else if file_type.is_file() {
                let file_name = entry.file_name().to_string_lossy().to_string();
                if file_name.ends_with(".aerofs.part") || file_name.contains(".aerofs-part-") {
                    let meta = entry.metadata().await?;
                    let mtime = meta.modified()?;
                    // A future timestamp can happen after a clock correction. It is not
                    // an I/O failure and should simply remain untouched until it ages out.
                    let Ok(age) = now.duration_since(mtime) else {
                        continue;
                    };
                    if age >= max_age {
                        tokio::fs::remove_file(&path).await?;
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

    Ok(deleted_count)
}
