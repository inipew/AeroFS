use crate::errors::VfsError;
use opendal::ErrorKind;

/// Convert an OpenDAL error into an AeroFS VfsError
pub fn map_opendal_error(err: opendal::Error, context: &str) -> VfsError {
    let msg = format!("{}: {}", context, err);
    match err.kind() {
        ErrorKind::NotFound => VfsError::NotFound(msg),
        ErrorKind::PermissionDenied => VfsError::PermissionDenied(msg),
        ErrorKind::AlreadyExists => VfsError::AlreadyExists(msg),
        ErrorKind::Unsupported => VfsError::NotSupported(msg),
        _ => VfsError::IoError(msg),
    }
}
