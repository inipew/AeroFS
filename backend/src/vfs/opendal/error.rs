use crate::errors::VfsError;
use opendal::ErrorKind;

/// Convert an OpenDAL error into an AeroFS VfsError with typed ErrorKind mapping
pub fn map_opendal_error(err: opendal::Error, context: &str) -> VfsError {
    let msg = format!("{}: {}", context, err);
    match err.kind() {
        ErrorKind::NotFound => VfsError::NotFound(msg),
        ErrorKind::PermissionDenied => VfsError::PermissionDenied(msg),
        ErrorKind::AlreadyExists => VfsError::AlreadyExists(msg),
        ErrorKind::Unsupported => VfsError::NotSupported(msg),
        ErrorKind::RateLimited => VfsError::RateLimited(msg),
        ErrorKind::IsADirectory => VfsError::NotAFile(msg),
        ErrorKind::NotADirectory => VfsError::NotADirectory(msg),
        ErrorKind::ConfigInvalid => VfsError::InvalidPath(msg),
        _ => {
            let err_str = err.to_string().to_lowercase();
            if err_str.contains("timeout") || err_str.contains("timed out") {
                VfsError::Timeout(msg)
            } else {
                VfsError::IoError(msg)
            }
        }
    }
}
