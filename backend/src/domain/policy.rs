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

/// Compatibility resolver for mutation flows that have not yet adopted strict
/// permission failure semantics. Provider failures are treated as no resolved
/// permission. New mutation code should use `resolve_destination_permissions_strict`.
pub async fn resolve_destination_permissions(
    dst_fs: &Arc<dyn FileSystem>,
    dst_vfs: &VfsPath,
    is_dir: bool,
    mode: PermissionInheritanceMode,
) -> Option<String> {
    match resolve_destination_permissions_strict(dst_fs, dst_vfs, is_dir, mode).await {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(path = %dst_vfs.path, ?error, "permission inheritance lookup failed");
            None
        }
    }
}

/// Strict permission resolver used by mutation paths where a provider lookup
/// failure must not be mistaken for an absent permission value.
pub async fn resolve_destination_permissions_strict(
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
            inherit_from_parent_strict(dst_fs, dst_vfs, is_dir).await
        }
        PermissionInheritanceMode::InheritParent => {
            inherit_from_parent_strict(dst_fs, dst_vfs, is_dir).await
        }
    }
}

async fn inherit_from_parent_strict(
    dst_fs: &Arc<dyn FileSystem>,
    dst_vfs: &VfsPath,
    is_dir: bool,
) -> Result<Option<String>, VfsError> {
    let Some(parent) = dst_vfs.parent() else {
        return Ok(None);
    };
    let parent_meta = match dst_fs.stat(&parent).await {
        Ok(metadata) => metadata,
        // `create_dir` may create missing ancestors. A missing parent therefore
        // means there is currently nothing to inherit, not that the provider is
        // unhealthy. Other stat failures remain observable to the caller.
        Err(VfsError::NotFound(_)) => return Ok(None),
        Err(error) => return Err(error),
    };
    let Some(parent_perms) = parent_meta.permissions else {
        return Ok(None);
    };

    let cleaned = parent_perms.trim_start_matches('0');
    if !cleaned.is_empty() {
        if let Ok(octal) = u32::from_str_radix(cleaned, 8) {
            if is_dir {
                return Ok(Some(format!("{:04o}", octal)));
            } else {
                let file_octal = octal & !0o111;
                return Ok(Some(format!("{:04o}", file_octal)));
            }
        }
    }

    Ok(Some(parent_perms))
}
