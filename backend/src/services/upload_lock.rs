use crate::domain::VfsPath;
use crate::errors::AppError;
use crate::transfer::TransferPlan;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Default)]
pub struct UploadLockManager {
    active_paths: Arc<Mutex<HashSet<String>>>,
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
    created_at: std::time::Instant,
    _guard: UploadGuard,
}

/// Cancellation-safe ownership token for an executing reserved upload.
///
/// The reservation remains in the manager while this token is alive. Dropping
/// the request future also drops this token, synchronously removing the session
/// and releasing the destination `UploadGuard`. This prevents claimed sessions
/// from pinning a destination forever when execution returns early or is
/// cancelled by a disconnected client.
#[derive(Debug)]
pub struct ClaimedUpload {
    job_id: String,
    session: UploadSession,
    manager: UploadLockManager,
}

impl ClaimedUpload {
    pub fn session(&self) -> &UploadSession {
        &self.session
    }
}

impl Drop for ClaimedUpload {
    fn drop(&mut self) {
        self.manager.release_sync(&self.job_id);
    }
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

    fn prune_stale_sync(&self) -> Result<(), AppError> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| AppError::Internal(anyhow::anyhow!("upload session manager poisoned")))?;
        let now = std::time::Instant::now();
        sessions.retain(|_, res| {
            res.claimed || now.duration_since(res.created_at) <= std::time::Duration::from_secs(60)
        });
        Ok(())
    }

    pub async fn prune_stale(&self) {
        if let Err(error) = self.prune_stale_sync() {
            tracing::error!(?error, "failed to prune stale upload reservations");
        }
    }

    pub async fn try_acquire(
        &self,
        connection_id: &str,
        path: &str,
    ) -> Result<UploadGuard, AppError> {
        self.prune_stale_sync()?;
        let normalized = format!("{}:{}", connection_id, path.trim_start_matches('/'));
        let mut lock = self
            .active_paths
            .lock()
            .map_err(|_| AppError::Internal(anyhow::anyhow!("upload lock manager poisoned")))?;
        if lock.contains(&normalized) {
            return Err(AppError::Conflict(format!(
                "A mutation is already in progress for destination path '{}'",
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
        self.reserve_existing(session, guard).await
    }

    /// Store a reservation after a caller has already acquired the path lock.
    pub async fn reserve_existing(
        &self,
        session: UploadSession,
        guard: UploadGuard,
    ) -> Result<(), AppError> {
        self.sessions
            .lock()
            .map_err(|_| AppError::Internal(anyhow::anyhow!("upload session manager poisoned")))?
            .insert(
                session.job_id.clone(),
                ReservedUpload {
                    session,
                    claimed: false,
                    created_at: std::time::Instant::now(),
                    _guard: guard,
                },
            );
        Ok(())
    }

    /// Claims a session once and returns a cancellation-safe ownership token.
    /// The destination remains reserved until the returned token is dropped.
    pub async fn claim(&self, job_id: &str, user_id: &str) -> Result<ClaimedUpload, AppError> {
        let session = {
            let mut sessions = self.sessions.lock().map_err(|_| {
                AppError::Internal(anyhow::anyhow!("upload session manager poisoned"))
            })?;
            let reserved = sessions.get_mut(job_id).ok_or_else(|| {
                AppError::NotFound(format!("Upload session '{}' not found", job_id))
            })?;
            if reserved.session.user_id != user_id {
                return Err(AppError::Forbidden(
                    "Cannot upload to another user's session".into(),
                ));
            }
            if reserved.claimed {
                return Err(AppError::Conflict(
                    "Upload session body has already been claimed".into(),
                ));
            }
            reserved.claimed = true;
            reserved.session.clone()
        };

        Ok(ClaimedUpload {
            job_id: job_id.to_string(),
            session,
            manager: self.clone(),
        })
    }

    fn release_sync(&self, job_id: &str) {
        match self.sessions.lock() {
            Ok(mut sessions) => {
                sessions.remove(job_id);
            }
            Err(_) => {
                tracing::error!(job_id, "upload session manager poisoned while releasing reservation");
            }
        }
    }

    pub async fn release(&self, job_id: &str) {
        self.release_sync(job_id);
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
        assert!(matches!(
            manager.claim("job_session", "bob").await,
            Err(AppError::Forbidden(_))
        ));

        let claimed = manager.claim("job_session", "alice").await.unwrap();
        assert_eq!(claimed.session().job_id, "job_session");
        assert!(matches!(
            manager.claim("job_session", "alice").await,
            Err(AppError::Conflict(_))
        ));

        drop(claimed);
        assert!(manager.try_acquire("local", "/session.bin").await.is_ok());
    }

    #[tokio::test]
    async fn dropping_claimed_upload_releases_destination_without_explicit_release() {
        let manager = UploadLockManager::new();
        manager
            .reserve(session("job_cancelled", "alice"))
            .await
            .unwrap();

        let claimed = manager.claim("job_cancelled", "alice").await.unwrap();
        assert!(manager.try_acquire("local", "/session.bin").await.is_err());

        drop(claimed);

        assert!(manager.try_acquire("local", "/session.bin").await.is_ok());
        assert!(matches!(
            manager.claim("job_cancelled", "alice").await,
            Err(AppError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn stale_unclaimed_session_expires_and_releases_lock() {
        let manager = UploadLockManager::new();
        let session = session("job_stale", "alice");
        let guard = manager.try_acquire("local", "/session.bin").await.unwrap();
        manager
            .sessions
            .lock()
            .unwrap()
            .insert(
                "job_stale".to_string(),
                ReservedUpload {
                    session,
                    claimed: false,
                    created_at: std::time::Instant::now()
                        - std::time::Duration::from_secs(70),
                    _guard: guard,
                },
            );
        assert!(manager.try_acquire("local", "/session.bin").await.is_ok());
    }
}
