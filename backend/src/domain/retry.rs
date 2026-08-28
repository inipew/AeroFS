use crate::errors::{AppError, ErrorCategory};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: usize,
    pub initial_interval: Duration,
    pub max_interval: Duration,
    pub multiplier: f64,
    pub jitter: bool,
    pub retryable_categories: Vec<ErrorCategory>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_interval: Duration::from_millis(500),
            max_interval: Duration::from_secs(10),
            multiplier: 2.0,
            jitter: true,
            retryable_categories: vec![
                ErrorCategory::Io,
                ErrorCategory::Provider,
                ErrorCategory::Timeout,
                ErrorCategory::RateLimited,
            ],
        }
    }
}

impl RetryPolicy {
    pub fn new(max_attempts: usize) -> Self {
        Self {
            max_attempts: max_attempts.max(1),
            ..Default::default()
        }
    }

    pub fn is_retryable(&self, err: &AppError) -> bool {
        match err {
            AppError::Auth(_)
            | AppError::Unauthorized(_)
            | AppError::Forbidden(_)
            | AppError::Security(_)
            | AppError::BadRequest(_)
            | AppError::NotFound(_)
            | AppError::PreconditionFailed(_)
            | AppError::RangeNotSatisfiable(_)
            | AppError::PayloadTooLarge(_)
            | AppError::InsufficientStorage(_) => false,
            AppError::ChecksumMismatch(_) => true,
            AppError::Vfs(vfs_err) => match vfs_err {
                crate::errors::VfsError::NotFound(_)
                | crate::errors::VfsError::PermissionDenied(_)
                | crate::errors::VfsError::AlreadyExists(_)
                | crate::errors::VfsError::InvalidPath(_)
                | crate::errors::VfsError::NotADirectory(_)
                | crate::errors::VfsError::NotAFile(_)
                | crate::errors::VfsError::DirectoryNotEmpty(_)
                | crate::errors::VfsError::NotSupported(_)
                | crate::errors::VfsError::QuotaExceeded(_)
                | crate::errors::VfsError::InsufficientStorage(_)
                | crate::errors::VfsError::Security(_) => false,
                crate::errors::VfsError::ConnectionError(_)
                | crate::errors::VfsError::IoError(_)
                | crate::errors::VfsError::RateLimited(_)
                | crate::errors::VfsError::Timeout(_)
                | crate::errors::VfsError::ChecksumMismatch(_) => true,
            },
            AppError::Conflict(_) => false,
            AppError::Internal(anyhow_err) => {
                let msg = anyhow_err.to_string().to_lowercase();
                !msg.contains("cancelled")
                    && !msg.contains("not found")
                    && !msg.contains("permission denied")
                    && !msg.contains("forbidden")
            }
        }
    }

    pub fn compute_backoff(&self, attempt: usize) -> Duration {
        if attempt <= 1 {
            return self.initial_interval;
        }
        let factor = self.multiplier.powi((attempt - 1) as i32);
        let millis = (self.initial_interval.as_millis() as f64 * factor) as u64;
        let clamped = Duration::from_millis(millis).min(self.max_interval);

        if self.jitter {
            let pseudo_random = ((attempt * 2654435761) % 250) as u64;
            clamped + Duration::from_millis(pseudo_random)
        } else {
            clamped
        }
    }
}
