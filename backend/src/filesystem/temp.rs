use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tokio::fs;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct TempFileManager {
    temp_root: PathBuf,
}

impl Default for TempFileManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TempFileManager {
    pub fn new() -> Self {
        let temp_root = std::env::temp_dir().join("aerofs_temp");
        if !temp_root.exists() {
            let _ = std::fs::create_dir_all(&temp_root);
        }
        Self { temp_root }
    }

    pub fn with_root(path: PathBuf) -> Self {
        if !path.exists() {
            let _ = std::fs::create_dir_all(&path);
        }
        Self { temp_root: path }
    }

    pub fn root(&self) -> &Path {
        &self.temp_root
    }

    /// Allocate a unique temporary file path with prefix
    pub fn create_temp_path(&self, prefix: &str, extension: Option<&str>) -> PathBuf {
        let id = Uuid::new_v4();
        let filename = match extension {
            Some(ext) => format!("{}_{}.{}", prefix, id, ext.trim_start_matches('.')),
            None => format!("{}_{}", prefix, id),
        };
        self.temp_root.join(filename)
    }

    /// Allocate and create a dedicated temporary directory
    pub async fn create_temp_dir(&self, prefix: &str) -> anyhow::Result<PathBuf> {
        let dir_path = self
            .temp_root
            .join(format!("{}_{}", prefix, Uuid::new_v4()));
        fs::create_dir_all(&dir_path).await?;
        Ok(dir_path)
    }

    /// Clean up files and folders older than max_age (default: 24 hours)
    pub async fn cleanup_stale(&self, max_age: Duration) -> anyhow::Result<usize> {
        let mut cleaned_count = 0;
        let mut entries = match fs::read_dir(&self.temp_root).await {
            Ok(e) => e,
            Err(_) => return Ok(0),
        };

        let now = SystemTime::now();

        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if let Ok(metadata) = entry.metadata().await {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(elapsed) = now.duration_since(modified) {
                        if elapsed > max_age {
                            if metadata.is_dir() {
                                if fs::remove_dir_all(&path).await.is_ok() {
                                    cleaned_count += 1;
                                }
                            } else {
                                if fs::remove_file(&path).await.is_ok() {
                                    cleaned_count += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(cleaned_count)
    }

    /// Spawn a periodic background cleanup task (every 6 hours)
    pub fn spawn_periodic_cleanup(&self, interval: Duration, max_age: Duration) {
        let manager = self.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                match manager.cleanup_stale(max_age).await {
                    Ok(count) => {
                        if count > 0 {
                            tracing::info!(
                                "TempFileManager cleaned up {} stale temporary files",
                                count
                            );
                        }
                    }
                    Err(e) => {
                        tracing::warn!("TempFileManager cleanup error: {}", e);
                    }
                }
            }
        });
    }
}
