use crate::domain::{Actor, ConnectionId, VfsPath};
use crate::errors::AppError;
use crate::filesystem::archive::{
    compress_zip, extract_selected_archive_entries, list_virtual_archive_entries,
    read_virtual_archive_entry, ArchiveFormat, ArchiveOverwriteMode, VirtualArchiveEntry,
};
use crate::filesystem::archive_compress_stream::compress_targz_streaming;
use crate::filesystem::archive_stream::{extract_targz_streaming, extract_zip_streaming};
use crate::ports::{
    archive::ArchiveEffects,
    authorization::{Authorization, FileAction},
    filesystem::FileSystemResolver,
};
use crate::runtime::{ResourceBudget, ResourceClass, ResourcePermit};
use crate::vfs::FileSystem;
use std::sync::Arc;

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct ArchiveResult {
    pub success: bool,
    pub message: String,
    pub entries_count: Option<usize>,
    pub skipped_count: Option<usize>,
}

#[derive(Clone)]
pub struct ArchiveService {
    authorization: Arc<dyn Authorization>,
    filesystem: Arc<dyn FileSystemResolver>,
    effects: Arc<dyn ArchiveEffects>,
    budget: Arc<ResourceBudget>,
}

impl ArchiveService {
    pub fn new(
        authorization: Arc<dyn Authorization>,
        filesystem: Arc<dyn FileSystemResolver>,
        effects: Arc<dyn ArchiveEffects>,
        budget: Arc<ResourceBudget>,
    ) -> Self {
        Self {
            authorization,
            filesystem,
            effects,
            budget,
        }
    }

    pub fn available_capacity(&self) -> usize {
        self.budget.available_archive()
    }

    async fn acquire_permit(
        &self,
        provider: &Arc<dyn FileSystem>,
    ) -> Result<ResourcePermit, AppError> {
        let class = if provider.is_local() {
            ResourceClass::ArchiveLocal
        } else {
            ResourceClass::ArchiveNetwork
        };
        self.budget.acquire(class).await.map_err(|_| {
            AppError::ServiceUnavailable("Archive resource budget is shutting down".into())
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn compress(
        &self,
        actor: &Actor,
        connection: &ConnectionId,
        base_path: &str,
        relative_paths: &[String],
        destination_file: &str,
        format_opt: Option<&str>,
    ) -> Result<ArchiveResult, AppError> {
        self.authorization
            .authorize(actor, connection, FileAction::Create)
            .await?;
        self.authorization
            .authorize(actor, connection, FileAction::Write)
            .await?;

        let provider = self.filesystem.resolve(connection).await?;
        let _permit = self.acquire_permit(&provider).await?;
        let dest_vfs = VfsPath::new(connection.as_str(), destination_file)?;
        let format = format_opt
            .and_then(ArchiveFormat::from_path)
            .or_else(|| ArchiveFormat::from_path(destination_file))
            .unwrap_or(ArchiveFormat::Zip);

        match format {
            ArchiveFormat::Zip => {
                compress_zip(
                    &provider,
                    connection.as_str(),
                    base_path,
                    relative_paths,
                    &dest_vfs,
                )
                .await?;
            }
            ArchiveFormat::TarGz => {
                compress_targz_streaming(
                    &provider,
                    connection.as_str(),
                    base_path,
                    relative_paths,
                    &dest_vfs,
                )
                .await?;
            }
        }

        self.effects
            .archive_created(actor, connection, &dest_vfs.path)
            .await;

        Ok(ArchiveResult {
            success: true,
            message: format!("Archive created: {}", dest_vfs.path),
            entries_count: Some(relative_paths.len()),
            skipped_count: None,
        })
    }

    pub async fn extract(
        &self,
        actor: &Actor,
        connection: &ConnectionId,
        archive_path: &str,
        destination_dir: &str,
        format_opt: Option<&str>,
        overwrite_mode: ArchiveOverwriteMode,
    ) -> Result<ArchiveResult, AppError> {
        self.authorization
            .authorize(actor, connection, FileAction::Create)
            .await?;
        self.authorization
            .authorize(actor, connection, FileAction::Write)
            .await?;

        let provider = self.filesystem.resolve(connection).await?;
        let _permit = self.acquire_permit(&provider).await?;
        let archive_vfs = VfsPath::new(connection.as_str(), archive_path)?;
        let format = format_opt
            .and_then(ArchiveFormat::from_path)
            .or_else(|| ArchiveFormat::from_path(archive_path))
            .unwrap_or(ArchiveFormat::Zip);

        let (count, skipped) = match format {
            ArchiveFormat::Zip => {
                extract_zip_streaming(&provider, &archive_vfs, destination_dir, overwrite_mode)
                    .await?
            }
            ArchiveFormat::TarGz => {
                extract_targz_streaming(&provider, &archive_vfs, destination_dir, overwrite_mode)
                    .await?
            }
        };

        self.effects
            .archive_extracted(
                actor,
                connection,
                &archive_vfs.path,
                destination_dir,
                count,
                skipped,
                false,
            )
            .await;

        Ok(ArchiveResult {
            success: true,
            message: format!("Extracted {} item(s) to {}", count, destination_dir),
            entries_count: Some(count),
            skipped_count: Some(skipped),
        })
    }

    pub async fn extract_selected(
        &self,
        actor: &Actor,
        connection: &ConnectionId,
        archive_path: &str,
        destination_dir: &str,
        entries: &[String],
        overwrite_mode: ArchiveOverwriteMode,
    ) -> Result<ArchiveResult, AppError> {
        self.authorization
            .authorize(actor, connection, FileAction::Create)
            .await?;
        self.authorization
            .authorize(actor, connection, FileAction::Write)
            .await?;

        let provider = self.filesystem.resolve(connection).await?;
        let _permit = self.acquire_permit(&provider).await?;
        let archive_vfs = VfsPath::new(connection.as_str(), archive_path)?;
        let (count, skipped) = extract_selected_archive_entries(
            &provider,
            &archive_vfs,
            destination_dir,
            entries,
            overwrite_mode,
        )
        .await?;

        self.effects
            .archive_extracted(
                actor,
                connection,
                &archive_vfs.path,
                destination_dir,
                count,
                skipped,
                true,
            )
            .await;

        Ok(ArchiveResult {
            success: true,
            message: format!("Extracted {} item(s) to {}", count, destination_dir),
            entries_count: Some(count),
            skipped_count: Some(skipped),
        })
    }

    pub async fn list_virtual(
        &self,
        actor: &Actor,
        connection: &ConnectionId,
        archive_path: &str,
        subpath: &str,
    ) -> Result<Vec<VirtualArchiveEntry>, AppError> {
        self.authorization
            .authorize(actor, connection, FileAction::Read)
            .await?;
        let provider = self.filesystem.resolve(connection).await?;
        let _permit = self.acquire_permit(&provider).await?;
        let archive_vfs = VfsPath::new(connection.as_str(), archive_path)?;
        Ok(list_virtual_archive_entries(&provider, &archive_vfs, subpath).await?)
    }

    pub async fn read_virtual_entry(
        &self,
        actor: &Actor,
        connection: &ConnectionId,
        archive_path: &str,
        entry_path: &str,
    ) -> Result<(String, Vec<u8>), AppError> {
        self.authorization
            .authorize(actor, connection, FileAction::Read)
            .await?;
        let provider = self.filesystem.resolve(connection).await?;
        let _permit = self.acquire_permit(&provider).await?;
        let archive_vfs = VfsPath::new(connection.as_str(), archive_path)?;
        Ok(read_virtual_archive_entry(&provider, &archive_vfs, entry_path).await?)
    }
}
