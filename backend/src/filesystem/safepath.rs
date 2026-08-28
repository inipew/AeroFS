use crate::domain::VfsPath;
use crate::errors::SecurityError;
use std::path::{Component, Path, PathBuf};

/// SafePath encapsulates a filesystem path that is verified to reside strictly inside
/// a designated root sandbox directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafePath {
    root: PathBuf,
    relative: PathBuf,
    absolute: PathBuf,
}

impl SafePath {
    /// Constructs a SafePath by resolving and validating a raw path against a sandbox root.
    ///
    /// If `allow_symlinks_outside` is false, any symlink resolving to a location outside the root
    /// will return `SecurityError::SymlinkEscape`.
    pub fn resolve(
        root_dir: &Path,
        input_path: &str,
        allow_symlinks_outside: bool,
    ) -> Result<Self, SecurityError> {
        // 1. Check for null byte injection
        if input_path.contains('\0') {
            return Err(SecurityError::InvalidPath(
                "Null byte detected in path".into(),
            ));
        }

        // 2. Ensure sandbox root exists or canonicalize it
        let canonical_root = root_dir
            .canonicalize()
            .map_err(|e| SecurityError::AccessDenied(format!("Sandbox root unavailable: {}", e)))?;

        // 3. Normalize relative path components lexically
        let mut clean_rel = PathBuf::new();
        for comp in Path::new(input_path).components() {
            match comp {
                Component::Normal(c) => clean_rel.push(c),
                Component::RootDir | Component::CurDir => continue,
                Component::ParentDir => {
                    // Prevent lexical traversal above root
                    if !clean_rel.pop() {
                        // Attempt to traverse above root
                        return Err(SecurityError::PathTraversal(format!(
                            "Traversal above sandbox root: {}",
                            input_path
                        )));
                    }
                }
                Component::Prefix(_) => {
                    return Err(SecurityError::InvalidPath(
                        "Windows prefix not allowed".into(),
                    ));
                }
            }
        }

        let absolute_target = canonical_root.join(&clean_rel);

        // 4. If target exists on disk, check canonical symlink resolution
        if absolute_target.exists() {
            let canonical_target = absolute_target.canonicalize().map_err(|e| {
                SecurityError::AccessDenied(format!("Failed to canonicalize target: {}", e))
            })?;

            if !allow_symlinks_outside && !canonical_target.starts_with(&canonical_root) {
                return Err(SecurityError::SymlinkEscape(format!(
                    "Path resolves outside sandbox: {}",
                    input_path
                )));
            }
        } else {
            // For non-existent files (e.g. creating new file), check parent directory if it exists
            if let Some(parent) = absolute_target.parent() {
                if parent.exists() {
                    let canonical_parent = parent.canonicalize().map_err(|e| {
                        SecurityError::AccessDenied(format!("Failed to canonicalize parent: {}", e))
                    })?;
                    if !allow_symlinks_outside && !canonical_parent.starts_with(&canonical_root) {
                        return Err(SecurityError::SymlinkEscape(format!(
                            "Parent directory resolves outside sandbox: {}",
                            input_path
                        )));
                    }
                }
            }
        }

        // Final verification that absolute path starts with canonical root
        if !absolute_target.starts_with(&canonical_root) {
            return Err(SecurityError::PathTraversal(format!(
                "Path escapes sandbox boundary: {}",
                input_path
            )));
        }

        Ok(Self {
            root: canonical_root,
            relative: clean_rel,
            absolute: absolute_target,
        })
    }

    /// Resolves from a `VfsPath`
    pub fn from_vfs_path(
        root_dir: &Path,
        vfs_path: &VfsPath,
        allow_symlinks_outside: bool,
    ) -> Result<Self, SecurityError> {
        Self::resolve(root_dir, &vfs_path.path, allow_symlinks_outside)
    }

    pub fn absolute(&self) -> &Path {
        &self.absolute
    }

    pub fn relative(&self) -> &Path {
        &self.relative
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Converts to standard relative path string with leading slash
    pub fn to_vfs_str(&self) -> String {
        let rel = self.relative.to_string_lossy();
        if rel.is_empty() || rel == "." {
            "/".to_string()
        } else {
            format!("/{}", rel.replace('\\', "/"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_safepath_normal() {
        let temp = tempdir().unwrap();
        let root = temp.path();

        let safe = SafePath::resolve(root, "foo/bar.txt", false).unwrap();
        assert_eq!(safe.to_vfs_str(), "/foo/bar.txt");
        assert!(safe.absolute().starts_with(root.canonicalize().unwrap()));

        let safe_root = SafePath::resolve(root, "/", false).unwrap();
        assert_eq!(safe_root.to_vfs_str(), "/");
    }

    #[test]
    fn test_safepath_traversal_attempts() {
        let temp = tempdir().unwrap();
        let root = temp.path();

        let err1 = SafePath::resolve(root, "../etc/passwd", false);
        assert!(err1.is_err());
        assert!(matches!(err1.unwrap_err(), SecurityError::PathTraversal(_)));

        let err2 = SafePath::resolve(root, "foo/../../etc/shadow", false);
        assert!(err2.is_err());
        assert!(matches!(err2.unwrap_err(), SecurityError::PathTraversal(_)));

        let err3 = SafePath::resolve(root, "/../../../../", false);
        assert!(err3.is_err());
    }

    #[test]
    fn test_safepath_null_byte() {
        let temp = tempdir().unwrap();
        let root = temp.path();

        let err = SafePath::resolve(root, "test.txt\0.png", false);
        assert!(err.is_err());
        assert!(matches!(err.unwrap_err(), SecurityError::InvalidPath(_)));
    }

    #[cfg(unix)]
    #[test]
    fn test_safepath_symlink_escape() {
        use std::os::unix::fs::symlink;

        let temp = tempdir().unwrap();
        let root = temp.path().join("sandbox");
        std::fs::create_dir_all(&root).unwrap();

        let outside_dir = temp.path().join("outside");
        std::fs::create_dir_all(&outside_dir).unwrap();
        let secret_file = outside_dir.join("secret.txt");
        std::fs::write(&secret_file, "confidential").unwrap();

        // Create symlink inside sandbox pointing to outside
        let link_path = root.join("escape_link");
        symlink(&outside_dir, &link_path).unwrap();

        // SafePath with allow_symlinks_outside = false MUST reject it
        let res = SafePath::resolve(&root, "escape_link/secret.txt", false);
        assert!(res.is_err());
        assert!(matches!(res.unwrap_err(), SecurityError::SymlinkEscape(_)));
    }
}
