use crate::domain::VfsPath;
use crate::errors::VfsError;
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

/// Resolves permission string for a destination path according to inheritance policy.
///
/// `Ok(None)` means the provider/policy has no permission value to apply. Provider
/// failures are propagated instead of being mistaken for an absent permission value.
pub async fn resolve_destination_permissions(
    dst_fs: &Arc<dyn FileSystem>,
    dst_vfs: &VfsPath,
    is_dir: bool,
    mode: PermissionInheritanceMode,
) -> Result<Option<String>, VfsError> {
    match mode {
        PermissionInheritanceMode::ProviderDefault => Ok(None),
        PermissionInheritanceMode::InheritExistingOrParent => {
            match dst_fs.stat(dst_vfs).await {
                Ok(existing_meta) => {
                    if let Some(perms) = existing_meta.permissions {
                        return Ok(Some(perms));
                    }
                }
                Err(VfsError::NotFound(_)) => {}
                Err(error) => return Err(error),
            }
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
) -> Result<Option<String>, VfsError> {
    let Some(parent) = dst_vfs.parent() else {
        return Ok(None);
    };
    let parent_meta = dst_fs.stat(&parent).await?;
    let Some(parent_perms) = parent_meta.permissions else {
        return Ok(None);
    };

    // Parse unix octal if available (e.g. "0755", "755")
    let cleaned = parent_perms.trim_start_matches('0');
    if !cleaned.is_empty() {
        if let Ok(octal) = u32::from_str_radix(cleaned, 8) {
            if is_dir {
                return Ok(Some(format!("{:04o}", octal)));
            } else {
                // For files, mask out execute bit from directory mode (e.g. 0755 -> 0644)
                let file_octal = octal & !0o111;
                return Ok(Some(format!("{:04o}", file_octal)));
            }
        }
    }

    Ok(Some(parent_perms))
}
