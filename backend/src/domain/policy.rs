use crate::domain::VfsPath;
use crate::vfs::FileSystem;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PermissionInheritanceMode {
    #[default]
    InheritExistingOrParent,
    InheritParent,
    ProviderDefault,
}

/// Resolves permission string for a destination path according to inheritance policy
pub async fn resolve_destination_permissions(
    dst_fs: &Arc<dyn FileSystem>,
    dst_vfs: &VfsPath,
    is_dir: bool,
    mode: PermissionInheritanceMode,
) -> Option<String> {
    match mode {
        PermissionInheritanceMode::ProviderDefault => None,
        PermissionInheritanceMode::InheritExistingOrParent => {
            // 1. If target already exists, preserve its current permissions
            if let Ok(existing_meta) = dst_fs.stat(dst_vfs).await {
                if let Some(perms) = existing_meta.permissions {
                    return Some(perms);
                }
            }
            // 2. Otherwise inherit from parent directory
            inherit_from_parent(dst_fs, dst_vfs, is_dir).await
        }
        PermissionInheritanceMode::InheritParent => {
            inherit_from_parent(dst_fs, dst_vfs, is_dir).await
        }
    }
}

async fn inherit_from_parent(
    dst_fs: &Arc<dyn FileSystem>,
    dst_vfs: &VfsPath,
    is_dir: bool,
) -> Option<String> {
    let parent = dst_vfs.parent()?;
    let parent_meta = dst_fs.stat(&parent).await.ok()?;
    let parent_perms = parent_meta.permissions?;

    // Parse unix octal if available (e.g. "0755", "755")
    let cleaned = parent_perms.trim_start_matches('0');
    if !cleaned.is_empty() {
        if let Ok(octal) = u32::from_str_radix(cleaned, 8) {
            if is_dir {
                return Some(format!("{:04o}", octal));
            } else {
                // For files, mask out execute bit from directory mode (e.g. 0755 -> 0644)
                let file_octal = octal & !0o111;
                return Some(format!("{:04o}", file_octal));
            }
        }
    }

    Some(parent_perms)
}
