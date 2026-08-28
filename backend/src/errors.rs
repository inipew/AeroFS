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

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ErrorResponse {
    pub error: ErrorDetail,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            AppError::Auth(AuthError::InvalidCredentials) => (
                StatusCode::UNAUTHORIZED,
                "INVALID_CREDENTIALS",
                self.to_string(),
            ),
            AppError::Auth(AuthError::SessionExpired) => (
                StatusCode::UNAUTHORIZED,
                "SESSION_EXPIRED",
                self.to_string(),
            ),
            AppError::Unauthorized(msg) => (
                StatusCode::UNAUTHORIZED,
                "UNAUTHORIZED",
                msg.clone(),
            ),
            AppError::Auth(AuthError::Unauthorized(_)) => {
                (StatusCode::FORBIDDEN, "FORBIDDEN", self.to_string())
            }
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, "FORBIDDEN", msg.clone()),
            AppError::Security(_) => (StatusCode::FORBIDDEN, "SECURITY_VIOLATION", self.to_string()),
            AppError::Vfs(VfsError::NotFound(_)) | AppError::NotFound(_) => {
                (StatusCode::NOT_FOUND, "NOT_FOUND", self.to_string())
            }
            AppError::Vfs(VfsError::PermissionDenied(_)) => (
                StatusCode::FORBIDDEN,
                "PERMISSION_DENIED",
                self.to_string(),
            ),
            AppError::Vfs(VfsError::AlreadyExists(_)) => {
                (StatusCode::CONFLICT, "ALREADY_EXISTS", self.to_string())
            }
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "BAD_REQUEST", msg.clone()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, "CONFLICT", msg.clone()),
            AppError::PayloadTooLarge(msg) => (StatusCode::PAYLOAD_TOO_LARGE, "PAYLOAD_TOO_LARGE", msg.clone()),
            AppError::InsufficientStorage(msg) => (StatusCode::INSUFFICIENT_STORAGE, "INSUFFICIENT_STORAGE", msg.clone()),
            AppError::Vfs(VfsError::InsufficientStorage(msg)) => (StatusCode::INSUFFICIENT_STORAGE, "INSUFFICIENT_STORAGE", msg.clone()),
            AppError::ChecksumMismatch(msg) => (StatusCode::UNPROCESSABLE_ENTITY, "CHECKSUM_MISMATCH", msg.clone()),
            AppError::Vfs(VfsError::ChecksumMismatch(msg)) => (StatusCode::UNPROCESSABLE_ENTITY, "CHECKSUM_MISMATCH", msg.clone()),
            AppError::Vfs(vfs_err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "VFS_ERROR",
                vfs_err.to_string(),
            ),
            AppError::Internal(err) => {
                let err_msg = format!("Internal error: {:?}", err);
                tracing::error!("{}", err_msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    err_msg,
                )
            }
        };

        let body = Json(ErrorResponse {
            error: ErrorDetail {
                code: code.to_string(),
                message,
                details: None,
            },
        });

        (status, body).into_response()
    }
}
