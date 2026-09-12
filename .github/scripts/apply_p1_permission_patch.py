from pathlib import Path


def replace_exact(text: str, old: str, new: str, count: int) -> str:
    actual = text.count(old)
    if actual != count:
        raise SystemExit(
            f"expected {count} occurrences, found {actual}: {old[:100]!r}"
        )
    return text.replace(old, new)


engine_path = Path("backend/src/transfer/engine.rs")
archive_path = Path("backend/src/filesystem/archive.rs")

engine = engine_path.read_text()
archive = archive_path.read_text()

engine_old_preflight = '''        // 1. Resolve destination permissions according to inheritance policy
        if let Some(perms) = crate::domain::resolve_destination_permissions(
            &dst_fs,
            &dst_vfs,
            false,
            crate::domain::PermissionInheritanceMode::InheritExistingOrParent,
        )
        .await
        {
            let _ = dst_fs.set_permissions(&dst_vfs, &perms).await;
        }
'''
engine_new_preflight = '''        // Resolve destination permissions once before mutation. This snapshot is then
        // applied to the staging artifact before promotion, or to the final destination
        // for direct-write providers.
        let target_perms = crate::domain::resolve_destination_permissions_strict(
            &dst_fs,
            &dst_vfs,
            false,
            crate::domain::PermissionInheritanceMode::InheritExistingOrParent,
        )
        .await
        .map_err(|error| {
            anyhow::anyhow!(
                "Destination permission lookup failed for '{}': {}",
                dst_vfs.path,
                error
            )
        })?;
'''
engine = replace_exact(engine, engine_old_preflight, engine_new_preflight, 1)

engine_old_commit = '''        // Atomically promote part file to destination path ONLY if staging was used
        if use_staging {
            if let Err(e) = dst_fs.rename(&write_target_vfs, &dst_vfs).await {
                let _ = dst_fs.delete(&write_target_vfs).await;
                return Err(anyhow::anyhow!(
                    "Failed to promote staging file to final destination '{}': {}",
                    dst_vfs.path,
                    e
                ));
            }
        }

        if let Some(perms) = crate::domain::resolve_destination_permissions(
            &dst_fs,
            &dst_vfs,
            false,
            crate::domain::PermissionInheritanceMode::InheritExistingOrParent,
        )
        .await
        {
            let _ = dst_fs.set_permissions(&dst_vfs, &perms).await;
        }
'''
engine_new_commit = '''        // Staged transfers apply inherited permissions before final promotion so the
        // destination never becomes visible with the wrong security metadata.
        if use_staging {
            if let Some(perms) = target_perms.as_deref() {
                if let Err(error) = dst_fs.set_permissions(&write_target_vfs, perms).await {
                    let _ = dst_fs.delete(&write_target_vfs).await;
                    return Err(anyhow::anyhow!(
                        "Failed to apply inherited permissions '{}' to staging file '{}': {}",
                        perms,
                        write_target_vfs.path,
                        error
                    ));
                }
            }

            if let Err(e) = dst_fs.rename(&write_target_vfs, &dst_vfs).await {
                let _ = dst_fs.delete(&write_target_vfs).await;
                return Err(anyhow::anyhow!(
                    "Failed to promote staging file to final destination '{}': {}",
                    dst_vfs.path,
                    e
                ));
            }
        } else if let Some(perms) = target_perms.as_deref() {
            if let Err(error) = dst_fs.set_permissions(&dst_vfs, perms).await {
                return Err(anyhow::anyhow!(
                    "Transfer content for '{}' was written, but applying inherited permissions '{}' failed: {}. Filesystem mutation committed; recovery required",
                    dst_vfs.path,
                    perms,
                    error
                ));
            }
        }
'''
engine = replace_exact(engine, engine_old_commit, engine_new_commit, 1)

archive_enum = '''#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ArchiveOverwriteMode {
    #[default]
    Overwrite,
    Skip,
    KeepBoth,
}
'''
archive_helpers = archive_enum + '''
async fn path_exists(
    provider: &Arc<dyn FileSystem>,
    path: &VfsPath,
) -> Result<bool, VfsError> {
    match provider.stat(path).await {
        Ok(_) => Ok(true),
        Err(VfsError::NotFound(_)) => Ok(false),
        Err(error) => Err(error),
    }
}

async fn ensure_directory(
    provider: &Arc<dyn FileSystem>,
    path: &VfsPath,
) -> Result<(), VfsError> {
    match provider.create_dir(path).await {
        Ok(()) => Ok(()),
        Err(create_error) => match provider.stat(path).await {
            Ok(metadata) if metadata.kind == FileKind::Directory => Ok(()),
            Ok(_) => Err(VfsError::IoError(format!(
                "Destination '{}' exists but is not a directory",
                path.path
            ))),
            Err(VfsError::NotFound(_)) => Err(create_error),
            Err(stat_error) => Err(stat_error),
        },
    }
}

async fn apply_committed_permissions(
    provider: &Arc<dyn FileSystem>,
    path: &VfsPath,
    permissions: Option<&str>,
    mutation: &str,
) -> Result<(), VfsError> {
    if let Some(permissions) = permissions {
        provider
            .set_permissions(path, permissions)
            .await
            .map_err(|error| {
                VfsError::IoError(format!(
                    "Archive {} '{}' was committed, but applying inherited permissions '{}' failed: {}. Filesystem mutation committed; recovery required",
                    mutation,
                    path.path,
                    permissions,
                    error
                ))
            })?;
    }
    Ok(())
}
'''
if "async fn path_exists(" in archive:
    raise SystemExit("archive permission helpers already present")
archive = replace_exact(archive, archive_enum, archive_helpers, 1)

archive_old_dir = '''            let _ = provider.create_dir(&dest_vfs).await;
            if let Some(perms) = crate::domain::resolve_destination_permissions(
                provider,
                &dest_vfs,
                true,
                crate::domain::PermissionInheritanceMode::InheritParent,
            )
            .await
            {
                let _ = provider.set_permissions(&dest_vfs, &perms).await;
            }
'''
archive_new_dir = '''            let dir_perms = crate::domain::resolve_destination_permissions_strict(
                provider,
                &dest_vfs,
                true,
                crate::domain::PermissionInheritanceMode::InheritParent,
            )
            .await?;
            ensure_directory(provider, &dest_vfs).await?;
            apply_committed_permissions(
                provider,
                &dest_vfs,
                dir_perms.as_deref(),
                "directory",
            )
            .await?;
'''
archive = replace_exact(archive, archive_old_dir, archive_new_dir, 3)

archive = replace_exact(
    archive,
    "            let exists = provider.stat(&dest_vfs).await.is_ok();\n",
    "            let exists = path_exists(provider, &dest_vfs).await?;\n",
    3,
)

archive_old_candidate = '''                            if provider.stat(&cand_vfs).await.is_err() {
                                break;
                            }
'''
archive_new_candidate = '''                            if !path_exists(provider, &cand_vfs).await? {
                                break;
                            }
'''
archive = replace_exact(archive, archive_old_candidate, archive_new_candidate, 3)

archive_old_parent = '''            if let Some(parent) = final_dest_vfs.parent() {
                let _ = provider.create_dir(&parent).await;
            }
'''
archive_new_parent = '''            let file_perms = crate::domain::resolve_destination_permissions_strict(
                provider,
                &final_dest_vfs,
                false,
                crate::domain::PermissionInheritanceMode::InheritExistingOrParent,
            )
            .await?;
            if let Some(parent) = final_dest_vfs.parent() {
                ensure_directory(provider, &parent).await?;
            }
'''
archive = replace_exact(archive, archive_old_parent, archive_new_parent, 3)

archive_old_postwrite = '''            if let Some(perms) = crate::domain::resolve_destination_permissions(
                provider,
                &final_dest_vfs,
                false,
                crate::domain::PermissionInheritanceMode::InheritExistingOrParent,
            )
            .await
            {
                let _ = provider.set_permissions(&final_dest_vfs, &perms).await;
            }
'''
archive_new_postwrite = '''            apply_committed_permissions(
                provider,
                &final_dest_vfs,
                file_perms.as_deref(),
                "file",
            )
            .await?;
'''
archive = replace_exact(archive, archive_old_postwrite, archive_new_postwrite, 3)

if "resolve_destination_permissions_strict" not in engine:
    raise SystemExit("engine strict permission resolver missing after patch")
if "let _ = dst_fs.set_permissions(&dst_vfs, &perms).await" in engine:
    raise SystemExit("engine still contains swallowed destination permission failure")
chmod_pos = engine.find("dst_fs.set_permissions(&write_target_vfs, perms)")
rename_pos = engine.find("dst_fs.rename(&write_target_vfs, &dst_vfs)")
if chmod_pos < 0 or rename_pos < 0 or chmod_pos >= rename_pos:
    raise SystemExit("engine staging chmod must occur before rename")
if "Filesystem mutation committed; recovery required" not in engine:
    raise SystemExit("engine partial-commit recovery marker missing")

if "resolve_destination_permissions_strict" not in archive:
    raise SystemExit("archive strict permission resolver missing after patch")
if "let _ = provider.set_permissions" in archive:
    raise SystemExit("archive still contains swallowed permission failure")
if "provider.stat(&dest_vfs).await.is_ok()" in archive:
    raise SystemExit("archive still collapses destination stat errors")
if "provider.stat(&cand_vfs).await.is_err()" in archive:
    raise SystemExit("archive still collapses keep-both stat errors")

engine_path.write_text(engine)
archive_path.write_text(archive)
