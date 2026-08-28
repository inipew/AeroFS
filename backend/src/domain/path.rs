use crate::errors::VfsError;
use serde::{Deserialize, Serialize};
use std::fmt;
use utoipa::ToSchema;

/// Universal identifier for a resource across any VFS provider.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub struct VfsPath {
    pub connection_id: String,
    pub path: String,
}

impl VfsPath {
    /// Safe parser that strictly rejects traversal attempts (`..`), null bytes, or drive prefixes
    pub fn parse(connection_id: impl Into<String>, path: &str) -> Result<Self, VfsError> {
        let normalized = Self::validate_and_normalize(path)?;
        Ok(Self {
            connection_id: connection_id.into(),
            path: normalized,
        })
    }

    /// Strict constructor guaranteeing path normalization and safety invariants
    pub fn new(
        connection_id: impl Into<String>,
        path: impl Into<String>,
    ) -> Result<Self, VfsError> {
        let raw_path = path.into();
        Self::parse(connection_id, &raw_path)
    }

    /// Root path for a given connection
    pub fn root(connection_id: impl Into<String>) -> Self {
        Self {
            connection_id: connection_id.into(),
            path: "/".to_string(),
        }
    }

    /// Validate that path does not contain traversal escapes (`..`), null bytes, or drive prefixes,
    /// and normalize duplicate slashes and dots.
    pub fn validate_and_normalize(p: &str) -> Result<String, VfsError> {
        if p.contains('\0') {
            return Err(VfsError::InvalidPath("Null byte detected in path".into()));
        }

        let trimmed = p.trim();
        if trimmed.is_empty() || trimmed == "/" || trimmed == "." {
            return Ok("/".to_string());
        }

        let mut parts = Vec::new();
        for segment in trimmed.split(&['/', '\\'][..]) {
            match segment {
                "" | "." => continue,
                ".." => {
                    return Err(VfsError::InvalidPath(format!(
                        "Path traversal '..' is strictly prohibited in '{}'",
                        p
                    )));
                }
                seg => {
                    if seg.contains(':') {
                        return Err(VfsError::InvalidPath(format!(
                            "Drive prefix or invalid colon in path segment '{}'",
                            seg
                        )));
                    }
                    parts.push(seg);
                }
            }
        }

        if parts.is_empty() {
            Ok("/".to_string())
        } else {
            Ok(format!("/{}", parts.join("/")))
        }
    }

    /// Normalize path string safely without swallowing traversal errors
    pub fn normalize_path_str(p: &str) -> String {
        Self::validate_and_normalize(p).unwrap_or_else(|_| p.to_string())
    }

    /// Returns the parent VfsPath if not already root
    pub fn parent(&self) -> Option<VfsPath> {
        if self.is_root() {
            return None;
        }
        let pos = self.path.rfind('/')?;
        if pos == 0 {
            Some(VfsPath::root(&self.connection_id))
        } else {
            Some(VfsPath {
                connection_id: self.connection_id.clone(),
                path: self.path[..pos].to_string(),
            })
        }
    }

    /// Returns the file/directory name
    pub fn file_name(&self) -> Option<&str> {
        if self.is_root() {
            None
        } else {
            self.path.rsplit('/').next()
        }
    }

    /// Check if this is the root directory "/"
    pub fn is_root(&self) -> bool {
        self.path == "/"
    }

    /// Joins a relative path segment to this VfsPath safely
    pub fn join(&self, child: &str) -> Result<VfsPath, VfsError> {
        let clean_child = child.trim_start_matches('/');
        let combined = if self.is_root() {
            format!("/{}", clean_child)
        } else {
            format!("{}/{}", self.path.trim_end_matches('/'), clean_child)
        };
        Self::parse(&self.connection_id, &combined)
    }
}

impl fmt::Display for VfsPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.connection_id, self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vfs_path_normalization() {
        let p = VfsPath::parse("local", "/a/b/c/./d/").unwrap();
        assert_eq!(p.path, "/a/b/c/d");

        let p2 = VfsPath::parse("sftp", "docs/report.pdf").unwrap();
        assert_eq!(p2.path, "/docs/report.pdf");
        assert_eq!(p2.file_name(), Some("report.pdf"));
        assert_eq!(p2.parent().unwrap().path, "/docs");
    }

    #[test]
    fn test_vfs_path_traversal_rejection() {
        assert!(VfsPath::parse("local", "../../etc/passwd").is_err());
        assert!(VfsPath::parse("local", "/a/b/../../c").is_err());
        assert!(VfsPath::parse("local", "/../../../").is_err());
        assert!(VfsPath::parse("local", "C:\\Windows\\System32").is_err());
        assert!(VfsPath::parse("local", "/test\0null").is_err());
    }

    #[test]
    fn test_vfs_path_join_and_parent() {
        let base = VfsPath::new("local", "/var/www").unwrap();
        let child = base.join("index.html").unwrap();
        assert_eq!(child.path, "/var/www/index.html");
        assert_eq!(child.parent().unwrap().path, "/var/www");
    }
}
