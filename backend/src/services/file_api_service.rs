use crate::domain::operation::OperationIntentType;
use crate::domain::{Actor, ConnectionId, FileMetadata, VfsPath};
use crate::errors::AppError;
use crate::filesystem::safepath::SafePath;
use crate::ports::{
    authorization::{Authorization, FileAction},
    cache::FileMetadataCache,
    effects::{FileAccessEffects, FileMutationEffects},
    filesystem::{ConnectionStorageMetadata, FileSystemResolver},
    settings::FileSettings,
};
use std::sync::Arc;
use tokio::io::AsyncReadExt;

#[derive(Debug, Clone)]
pub struct RecursiveChmodResult {
    pub succeeded: usize,
    pub failed: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct StorageInfoSnapshot {
    pub source_name: String,
    pub source_size_formatted: String,
    pub disk_label: String,
    pub disk_usage_text: String,
    pub used_percent: u8,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub free_bytes: u64,
}

#[derive(Clone)]
pub struct FileApiService {
    authorization: Arc<dyn Authorization>,
    filesystem: Arc<dyn FileSystemResolver>,
    access_effects: Arc<dyn FileAccessEffects>,
    mutation_effects: Arc<dyn FileMutationEffects>,
    file_settings: Arc<dyn FileSettings>,
    connection_storage: Arc<dyn ConnectionStorageMetadata>,
    metadata_cache: Arc<dyn FileMetadataCache>,
}

impl FileApiService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        authorization: Arc<dyn Authorization>,
        filesystem: Arc<dyn FileSystemResolver>,
        access_effects: Arc<dyn FileAccessEffects>,
        mutation_effects: Arc<dyn FileMutationEffects>,
        file_settings: Arc<dyn FileSettings>,
        connection_storage: Arc<dyn ConnectionStorageMetadata>,
        metadata_cache: Arc<dyn FileMetadataCache>,
    ) -> Self {
        Self {
            authorization,
            filesystem,
            access_effects,
            mutation_effects,
            file_settings,
            connection_storage,
            metadata_cache,
        }
    }

    pub async fn cached_metadata(&self, connection_id: &str, path: &str) -> Option<FileMetadata> {
        self.metadata_cache.get(connection_id, path).await
    }

    pub async fn cache_metadata(&self, connection_id: &str, path: &str, metadata: FileMetadata) {
        self.metadata_cache.put(connection_id, path, metadata).await;
    }

    pub async fn read_text_for_editing(
        &self,
        connection: &ConnectionId,
        path: &str,
        size: u64,
    ) -> Result<String, AppError> {
        let max_editable_size = self.file_settings.max_editable_size().await?;
        if size > max_editable_size {
            return Err(AppError::PayloadTooLarge(format!(
                "File size ({} bytes) exceeds maximum editable size ({} bytes)",
                size, max_editable_size
            )));
        }
        let provider = self.filesystem.resolve(connection).await?;
        let vfs_path = VfsPath::new(connection.as_str(), path)?;
        let mut stream = provider.read_stream(&vfs_path).await?;
        let mut data = Vec::new();
        stream
            .read_to_end(&mut data)
            .await
            .map_err(|e| anyhow::anyhow!("Read error: {}", e))?;
        String::from_utf8(data)
            .map_err(|_| AppError::BadRequest("File contains non-UTF8 binary data".into()))
    }

    pub async fn authorize_intent(
        &self,
        actor: &Actor,
        intent: OperationIntentType,
        source: &ConnectionId,
        destination: Option<&ConnectionId>,
    ) -> Result<(), AppError> {
        match intent {
            OperationIntentType::Copy => {
                self.authorization.authorize(actor, source, FileAction::Read).await?;
                if let Some(dest) = destination {
                    self.authorization.authorize(actor, dest, FileAction::Create).await?;
                    self.authorization.authorize(actor, dest, FileAction::Write).await?;
                }
            }
            OperationIntentType::Move => {
                self.authorization.authorize(actor, source, FileAction::Read).await?;
                self.authorization.authorize(actor, source, FileAction::Delete).await?;
                if let Some(dest) = destination {
                    self.authorization.authorize(actor, dest, FileAction::Create).await?;
                    self.authorization.authorize(actor, dest, FileAction::Write).await?;
                }
            }
            OperationIntentType::Delete => {
                self.authorization.authorize(actor, source, FileAction::Delete).await?;
            }
            OperationIntentType::Chmod => {
                self.authorization.authorize(actor, source, FileAction::Write).await?;
            }
            OperationIntentType::Compress | OperationIntentType::Extract => {
                self.authorization.authorize(actor, source, FileAction::Read).await?;
                if let Some(dest) = destination {
                    self.authorization.authorize(actor, dest, FileAction::Create).await?;
                    self.authorization.authorize(actor, dest, FileAction::Write).await?;
                }
            }
        }
        Ok(())
    }

    pub async fn chmod_recursive(
        &self,
        actor: &Actor,
        connection: &ConnectionId,
        path: &str,
        mode: u32,
    ) -> Result<RecursiveChmodResult, AppError> {
        #[cfg(not(unix))]
        {
            let _ = (actor, connection, path, mode);
            return Err(AppError::BadRequest(
                "CHMOD is only supported on Unix systems".into(),
            ));
        }

        #[cfg(unix)]
        {
            if connection.as_str() != ConnectionId::LOCAL {
                return Err(AppError::BadRequest(
                    "Recursive CHMOD is only supported for local storage".into(),
                ));
            }

            self.authorization
                .authorize(actor, connection, FileAction::Write)
                .await?;

            let root = self.file_settings.local_root().await?;
            let allow_symlinks = self.file_settings.allow_symlinks_outside_root().await?;
            let safe_path = SafePath::resolve(&root, path, allow_symlinks)
                .map_err(|error| AppError::BadRequest(error.to_string()))?;

            let vfs_path = VfsPath::new(connection.as_str(), path)?;
            let provider = self.filesystem.resolve(connection).await?;
            provider
                .set_permissions(&vfs_path, &format!("{:04o}", mode))
                .await?;

            let mut result = RecursiveChmodResult {
                succeeded: 0,
                failed: Vec::new(),
            };
            let absolute = safe_path.absolute();
            if absolute.is_dir() {
                result = apply_chmod_recursive(absolute, mode).await;
            }

            self.mutation_effects
                .invalidate_prefix(connection, &vfs_path.path)
                .await;

            if result.failed.is_empty() {
                self.access_effects
                    .accessed(
                        actor,
                        connection,
                        &vfs_path.path,
                        "FILE_CHMOD",
                        Some(format!(
                            "Changed permissions to {:o} on {} recursively",
                            mode, vfs_path.path
                        )),
                    )
                    .await;
            }

            Ok(result)
        }
    }

    pub async fn storage_info(&self, connection_id: &str) -> StorageInfoSnapshot {
        if connection_id == ConnectionId::LOCAL {
            let root = match self.file_settings.local_root().await {
                Ok(root) => root,
                Err(error) => {
                    tracing::warn!(%error, "failed to resolve local root for storage info");
                    return local_storage_fallback();
                }
            };

            #[cfg(unix)]
            {
                let mut stat = std::mem::MaybeUninit::uninit();
                if let Ok(c_path) = std::ffi::CString::new(root.to_string_lossy().as_bytes()) {
                    if unsafe { libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) } == 0 {
                        let stat = unsafe { stat.assume_init() };
                        let total = stat.f_blocks * stat.f_frsize;
                        let free = stat.f_bavail * stat.f_frsize;
                        let used = total.saturating_sub(free);
                        let pct = if total > 0 {
                            ((used as f64 / total as f64) * 100.0) as u8
                        } else {
                            0
                        };
                        let total_gib = (total as f64) / (1024.0 * 1024.0 * 1024.0);
                        return StorageInfoSnapshot {
                            source_name: "Local Storage".to_string(),
                            source_size_formatted: format_bytes(used),
                            disk_label: "Disk".to_string(),
                            disk_usage_text: format!("{}% · {:.0} GiB", pct, total_gib),
                            used_percent: pct,
                            total_bytes: total,
                            used_bytes: used,
                            free_bytes: free,
                        };
                    }
                }
            }

            return local_storage_fallback();
        }

        match self.connection_storage.get(connection_id).await {
            Ok(Some(descriptor)) => {
                let port_str = descriptor
                    .port
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "21".into());
                let host_str = descriptor.host.unwrap_or_else(|| "Remote".into());
                StorageInfoSnapshot {
                    source_name: descriptor.name,
                    source_size_formatted: format!("{} Remote", descriptor.provider.to_uppercase()),
                    disk_label: format!("{}:{}", host_str, port_str),
                    disk_usage_text: "Connected · Online".to_string(),
                    used_percent: 0,
                    total_bytes: 0,
                    used_bytes: 0,
                    free_bytes: 0,
                }
            }
            Ok(None) => remote_storage_fallback(connection_id),
            Err(error) => {
                tracing::warn!(%error, connection_id, "failed to load remote storage metadata");
                remote_storage_fallback(connection_id)
            }
        }
    }
}

fn local_storage_fallback() -> StorageInfoSnapshot {
    StorageInfoSnapshot {
        source_name: "Local Storage".to_string(),
        source_size_formatted: "Local".to_string(),
        disk_label: "Disk".to_string(),
        disk_usage_text: "Available".to_string(),
        used_percent: 0,
        total_bytes: 0,
        used_bytes: 0,
        free_bytes: 0,
    }
}

fn remote_storage_fallback(connection_id: &str) -> StorageInfoSnapshot {
    StorageInfoSnapshot {
        source_name: connection_id.to_string(),
        source_size_formatted: "Remote".to_string(),
        disk_label: "Network".to_string(),
        disk_usage_text: "Connected".to_string(),
        used_percent: 0,
        total_bytes: 0,
        used_bytes: 0,
        free_bytes: 0,
    }
}

#[cfg(unix)]
async fn apply_chmod_recursive(dir: &std::path::Path, mode: u32) -> RecursiveChmodResult {
    use std::os::unix::fs::PermissionsExt;

    let mut succeeded = 0;
    let mut failed = Vec::new();
    let mut stack = vec![dir.to_path_buf()];

    while let Some(curr_dir) = stack.pop() {
        match tokio::fs::read_dir(&curr_dir).await {
            Ok(mut entries) => loop {
                match entries.next_entry().await {
                    Ok(Some(entry)) => {
                        let path = entry.path();
                        let perms = std::fs::Permissions::from_mode(mode);
                        match std::fs::set_permissions(&path, perms) {
                            Ok(_) => succeeded += 1,
                            Err(error) => failed.push(format!("{}: {}", path.display(), error)),
                        }
                        if path.is_dir() {
                            stack.push(path);
                        }
                    }
                    Ok(None) => break,
                    Err(error) => {
                        failed.push(format!("{}: {}", curr_dir.display(), error));
                        break;
                    }
                }
            },
            Err(error) => failed.push(format!("{}: {}", curr_dir.display(), error)),
        }
    }

    RecursiveChmodResult { succeeded, failed }
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 * 1024 {
        format!(
            "{:.1} TiB",
            bytes as f64 / (1024.0 * 1024.0 * 1024.0 * 1024.0)
        )
    } else if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1} GiB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}
