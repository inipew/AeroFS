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
    pub fn new(connection_id: impl Into<String>, path: impl Into<String>) -> Self {
        let raw_path = path.into();
        let normalized = Self::normalize_path_str(&raw_path);
        Self {
            connection_id: connection_id.into(),
            path: normalized,
        }
    }

    /// Root path for a given connection
    pub fn root(connection_id: impl Into<String>) -> Self {
        Self {
            connection_id: connection_id.into(),
            path: "/".to_string(),
        }
    }

    /// Normalize path string to standard Unix-style leading slash without trailing slashes (except root "/")
    pub fn normalize_path_str(p: &str) -> String {
        let trimmed = p.trim();
        if trimmed.is_empty() || trimmed == "/" || trimmed == "." {
            return "/".to_string();
        }

        let mut parts = Vec::new();
        for segment in trimmed.split(&['/', '\\'][..]) {
            match segment {
                "" | "." => continue,
                ".." => {
                    parts.pop();
                }
                seg => parts.push(seg),
            }
        }

        if parts.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", parts.join("/"))
        }
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

    /// Joins a relative path segment to this VfsPath
    pub fn join(&self, child: &str) -> VfsPath {
        if self.is_root() {
            VfsPath::new(&self.connection_id, child)
        } else {
            let combined = format!("{}/{}", self.path, child.trim_start_matches('/'));
            VfsPath::new(&self.connection_id, combined)
        }
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
        let p = VfsPath::new("local", "/a/b/../c/./d/");
        assert_eq!(p.path, "/a/c/d");

        let root = VfsPath::new("local", "/../../../");
        assert_eq!(root.path, "/");
        assert!(root.is_root());

        let p2 = VfsPath::new("sftp", "docs/report.pdf");
        assert_eq!(p2.path, "/docs/report.pdf");
        assert_eq!(p2.file_name(), Some("report.pdf"));
        assert_eq!(p2.parent().unwrap().path, "/docs");
    }

    #[test]
    fn test_vfs_path_join_and_parent() {
        let base = VfsPath::new("local", "/var/www");
        let child = base.join("index.html");
        assert_eq!(child.path, "/var/www/index.html");
        assert_eq!(child.parent().unwrap().path, "/var/www");
    }
}
