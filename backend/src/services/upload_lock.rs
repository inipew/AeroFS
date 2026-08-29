use crate::errors::AppError;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Default)]
pub struct UploadLockManager {
    active_paths: Arc<Mutex<HashSet<String>>>,
}

#[derive(Debug)]
pub struct UploadGuard {
    key: String,
    manager: UploadLockManager,
}

impl Drop for UploadGuard {
    fn drop(&mut self) {
        let key = self.key.clone();
        let active = Arc::clone(&self.manager.active_paths);
        tokio::spawn(async move {
            let mut lock = active.lock().await;
            lock.remove(&key);
        });
    }
}

impl UploadLockManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn try_acquire(
        &self,
        connection_id: &str,
        path: &str,
    ) -> Result<UploadGuard, AppError> {
        let normalized = format!("{}:{}", connection_id, path.trim_start_matches('/'));
        let mut lock = self.active_paths.lock().await;
        if lock.contains(&normalized) {
            return Err(AppError::Conflict(format!(
                "An upload is already in progress for destination path '{}'",
                path
            )));
        }
        lock.insert(normalized.clone());
        Ok(UploadGuard {
            key: normalized,
            manager: self.clone(),
        })
    }
}
