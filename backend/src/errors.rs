use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use utoipa::ToSchema;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("VFS error: {0}")]
    Vfs(#[from] VfsError),

    #[error("Security error: {0}")]
    Security(#[from] SecurityError),

    #[error("Authentication error: {0}")]
    Auth(#[from] AuthError),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Precondition failed: {0}")]
    PreconditionFailed(String),

    #[error("Range not satisfiable: {0}")]
    RangeNotSatisfiable(String),

    #[error("Payload too large: {0}")]
    PayloadTooLarge(String),

    #[error("Insufficient storage: {0}")]
    InsufficientStorage(String),

    #[error("Checksum mismatch: {0}")]
    ChecksumMismatch(String),

    #[error("Internal server error: {0}")]
    Internal(#[from] anyhow::Error),
}

#[derive(Debug, Error)]
pub enum VfsError {
    #[error("File or directory not found: {0}")]
    NotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Already exists: {0}")]
    AlreadyExists(String),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Not a directory: {0}")]
    NotADirectory(String),

    #[error("Not a file: {0}")]
    NotAFile(String),

    #[error("Directory not empty: {0}")]
    DirectoryNotEmpty(String),

    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Operation not supported: {0}")]
    NotSupported(String),

    #[error("I/O error: {0}")]
    IoError(String),

    #[error("Quota exceeded: {0}")]
    QuotaExceeded(String),

    #[error("Insufficient storage: {0}")]
    InsufficientStorage(String),

    #[error("Rate limited by storage provider: {0}")]
    RateLimited(String),

    #[error("Storage operation timed out: {0}")]
    Timeout(String),

    #[error("Checksum mismatch: {0}")]
    ChecksumMismatch(String),

    #[error("Security error: {0}")]
    Security(#[from] SecurityError),
}

#[derive(Debug, Error)]
pub enum SecurityError {
    #[error("Path traversal attempt detected: {0}")]
    PathTraversal(String),

    #[error("Symlink points outside sandbox: {0}")]
    SymlinkEscape(String),

    #[error("Invalid character or null byte in path")]
    NullByte,

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Access to restricted resource denied: {0}")]
    AccessDenied(String),

    #[error("SSRF attempt blocked: {0}")]
    SsrfBlocked(String),
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Session expired or invalid")]
    SessionExpired,

    #[error("Insufficient permissions: required {0}")]
    Unauthorized(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    Validation,
    Authentication,
    Authorization,
    NotFound,
    Conflict,
    PayloadTooLarge,
    InsufficientStorage,
    RateLimited,
    Timeout,
    Provider,
    Io,
    Security,
    Internal,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ErrorResponse {
    pub error: ErrorDetail,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ErrorDetail {
    pub code: String,
    pub category: ErrorCategory,
    pub retryable: bool,
    pub user_action: Option<String>,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, category, retryable, user_action, message) = match &self {
            AppError::Auth(AuthError::InvalidCredentials) => (
                StatusCode::UNAUTHORIZED,
                "INVALID_CREDENTIALS",
                ErrorCategory::Authentication,
                false,
                Some("check_username_and_password".to_string()),
                self.to_string(),
            ),
            AppError::Auth(AuthError::SessionExpired) => (
                StatusCode::UNAUTHORIZED,
                "SESSION_EXPIRED",
                ErrorCategory::Authentication,
                true,
                Some("re_login".to_string()),
                self.to_string(),
            ),
            AppError::Unauthorized(msg) => (
                StatusCode::UNAUTHORIZED,
                "UNAUTHORIZED",
                ErrorCategory::Authentication,
                false,
                Some("login_required".to_string()),
                msg.clone(),
            ),
            AppError::Auth(AuthError::Unauthorized(_)) => (
                StatusCode::FORBIDDEN,
                "FORBIDDEN",
                ErrorCategory::Authorization,
                false,
                Some("request_permission_from_admin".to_string()),
                self.to_string(),
            ),
            AppError::Forbidden(msg) => (
                StatusCode::FORBIDDEN,
                "FORBIDDEN",
                ErrorCategory::Authorization,
                false,
                Some("check_access_permissions".to_string()),
                msg.clone(),
            ),
            AppError::Security(_) => (
                StatusCode::FORBIDDEN,
                "SECURITY_VIOLATION",
                ErrorCategory::Security,
                false,
                Some("verify_path_and_target_boundaries".to_string()),
                self.to_string(),
            ),
            AppError::Vfs(VfsError::NotFound(_)) | AppError::NotFound(_) => (
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                ErrorCategory::NotFound,
                false,
                Some("verify_item_exists".to_string()),
                self.to_string(),
            ),
            AppError::Vfs(VfsError::PermissionDenied(_)) => (
                StatusCode::FORBIDDEN,
                "PERMISSION_DENIED",
                ErrorCategory::Authorization,
                false,
                Some("grant_filesystem_permissions".to_string()),
                self.to_string(),
            ),
            AppError::Vfs(VfsError::AlreadyExists(_)) => (
                StatusCode::CONFLICT,
                "ALREADY_EXISTS",
                ErrorCategory::Conflict,
                false,
                Some("rename_or_overwrite".to_string()),
                self.to_string(),
            ),
            AppError::BadRequest(msg) => (
                StatusCode::BAD_REQUEST,
                "BAD_REQUEST",
                ErrorCategory::Validation,
                false,
                Some("check_request_payload".to_string()),
                msg.clone(),
            ),
            AppError::Conflict(msg) => (
                StatusCode::CONFLICT,
                "CONFLICT",
                ErrorCategory::Conflict,
                false,
                Some("reload_latest_version_and_retry".to_string()),
                msg.clone(),
            ),
            AppError::PreconditionFailed(msg) => (
                StatusCode::PRECONDITION_FAILED,
                "PRECONDITION_FAILED",
                ErrorCategory::Conflict,
                false,
                Some("reload_latest_version_and_retry".to_string()),
                msg.clone(),
            ),
            AppError::RangeNotSatisfiable(msg) => (
                StatusCode::RANGE_NOT_SATISFIABLE,
                "RANGE_NOT_SATISFIABLE",
                ErrorCategory::Validation,
                false,
                Some("verify_range_bounds".to_string()),
                msg.clone(),
            ),
            AppError::PayloadTooLarge(msg) => (
                StatusCode::PAYLOAD_TOO_LARGE,
                "PAYLOAD_TOO_LARGE",
                ErrorCategory::PayloadTooLarge,
                false,
                Some("reduce_payload_size".to_string()),
                msg.clone(),
            ),
            AppError::InsufficientStorage(msg)
            | AppError::Vfs(VfsError::InsufficientStorage(msg)) => (
                StatusCode::INSUFFICIENT_STORAGE,
                "INSUFFICIENT_STORAGE",
                ErrorCategory::InsufficientStorage,
                false,
                Some("free_disk_space_and_retry".to_string()),
                msg.clone(),
            ),
            AppError::ChecksumMismatch(msg) | AppError::Vfs(VfsError::ChecksumMismatch(msg)) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "CHECKSUM_MISMATCH",
                ErrorCategory::Io,
                true,
                Some("retransfer_payload".to_string()),
                msg.clone(),
            ),
            AppError::Vfs(VfsError::RateLimited(msg)) => (
                StatusCode::TOO_MANY_REQUESTS,
                "RATE_LIMITED",
                ErrorCategory::RateLimited,
                true,
                Some("retry_after_backoff".to_string()),
                msg.clone(),
            ),
            AppError::Vfs(VfsError::Timeout(msg)) => (
                StatusCode::GATEWAY_TIMEOUT,
                "STORAGE_TIMEOUT",
                ErrorCategory::Timeout,
                true,
                Some("retry_operation".to_string()),
                msg.clone(),
            ),
            AppError::Vfs(vfs_err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "VFS_ERROR",
                ErrorCategory::Provider,
                true,
                Some("check_provider_status".to_string()),
                vfs_err.to_string(),
            ),
            AppError::Internal(err) => {
                let err_msg = format!("Internal error: {:?}", err);
                tracing::error!("{}", err_msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    ErrorCategory::Internal,
                    false,
                    Some("contact_system_administrator".to_string()),
                    err_msg,
                )
            }
        };

        let body = Json(ErrorResponse {
            error: ErrorDetail {
                code: code.to_string(),
                category,
                retryable,
                user_action,
                message,
                details: None,
            },
        });

        (status, body).into_response()
    }
}
