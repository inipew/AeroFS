use crate::errors::AppError;
use crate::domain::VfsPath;
use crate::transfer::TransferPlan;
use std::collections::HashSet;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Default)]
pub struct UploadLockManager {
    active_paths: Arc<std::sync::Mutex<HashSet<String>>>,
    sessions: Arc<Mutex<HashMap<String, ReservedUpload>>>,
}

/// Admission data retained between `POST /uploads` and its streaming PUT.
/// The contained guard deliberately owns the destination reservation until the
/// job reaches a terminal state.
#[derive(Debug, Clone)]
pub struct UploadSession {
    pub job_id: String,
    pub user_id: String,
    pub connection_id: String,
    pub target: VfsPath,
    pub file_name: String,
    pub total_bytes: Option<u64>,
    pub max_upload_bytes: u64,
    pub target_exists: bool,
    pub target_perms: Option<String>,
    pub plan: TransferPlan,
}

#[derive(Debug)]
struct ReservedUpload {
    session: UploadSession,
    claimed: bool,
    _guard: UploadGuard,
}

#[derive(Debug)]
pub struct UploadGuard {
    key: String,
    manager: UploadLockManager,
}

impl Drop for UploadGuard {
    fn drop(&mut self) {
        // This lock is intentionally synchronous and held only for a HashSet
        // mutation, so releasing a reservation is immediate and deterministic.
        if let Ok(mut active) = self.manager.active_paths.lock() {
            active.remove(&self.key);
        }
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
        let mut lock = self.active_paths.lock().map_err(|_| {
            AppError::Internal(anyhow::anyhow!("upload lock manager poisoned"))
        })?;
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

    pub async fn reserve(&self, session: UploadSession) -> Result<(), AppError> {
        let guard = self
            .try_acquire(&session.connection_id, &session.target.path)
            .await?;
        self.sessions.lock().await.insert(
            session.job_id.clone(),
            ReservedUpload {
                session,
                claimed: false,
                _guard: guard,
            },
        );
        Ok(())
    }

    /// Store a reservation after a caller has already acquired the path lock.
    pub async fn reserve_existing(&self, session: UploadSession, guard: UploadGuard) {
        self.sessions.lock().await.insert(
            session.job_id.clone(),
            ReservedUpload {
                session,
                claimed: false,
                _guard: guard,
            },
        );
    }

    /// Claims a session once. The reservation remains held while its body is
    /// executing, preventing duplicate PUTs or a competing destination write.
    pub async fn claim(&self, job_id: &str, user_id: &str) -> Result<UploadSession, AppError> {
        let mut sessions = self.sessions.lock().await;
        let reserved = sessions
            .get_mut(job_id)
            .ok_or_else(|| AppError::NotFound(format!("Upload session '{}' not found", job_id)))?;
        if reserved.session.user_id != user_id {
            return Err(AppError::Forbidden("Cannot upload to another user's session".into()));
        }
        if reserved.claimed {
            return Err(AppError::Conflict("Upload session body has already been claimed".into()));
        }
        reserved.claimed = true;
        Ok(reserved.session.clone())
    }

    pub async fn release(&self, job_id: &str) {
        self.sessions.lock().await.remove(job_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::CommitSemantics;
    use crate::transfer::{TransferExecutionMode, TransferStaging};

    fn session(job_id: &str, user_id: &str) -> UploadSession {
        UploadSession {
            job_id: job_id.into(),
            user_id: user_id.into(),
            connection_id: "local".into(),
            target: VfsPath::new("local", "/session.bin").unwrap(),
            file_name: "session.bin".into(),
            total_bytes: Some(1),
            max_upload_bytes: 10,
            target_exists: false,
            target_perms: None,
            plan: TransferPlan {
                execution_mode: TransferExecutionMode::Inline,
                staging: TransferStaging::LocalTemp,
                commit: CommitSemantics::AtomicRename,
            },
        }
    }

    #[tokio::test]
    async fn session_is_owned_claimed_once_and_releases_destination() {
        let manager = UploadLockManager::new();
        let session = session("job_session", "alice");
        manager.reserve(session.clone()).await.unwrap();
        assert!(matches!(manager.claim("job_session", "bob").await, Err(AppError::Forbidden(_))));
        assert_eq!(manager.claim("job_session", "alice").await.unwrap().job_id, "job_session");
        assert!(matches!(manager.claim("job_session", "alice").await, Err(AppError::Conflict(_))));
        manager.release("job_session").await;
        assert!(manager.try_acquire("local", "/session.bin").await.is_ok());
    }
}
