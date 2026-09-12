//! Upload application orchestration.
//! HTTP adapts request bodies into byte streams; this module owns admission,
//! mutation ownership and post-commit effects through application ports.

use crate::domain::{Actor, ConnectionId, PermissionInheritanceMode, VfsPath};
use crate::errors::{AppError, VfsError};
use crate::ports::{
    authorization::{Authorization, FileAction},
    effects::FileMutationEffects,
    filesystem::FileSystemResolver,
    mutation::MutationCoordinator,
    upload::{
        CreateInlineUploadJob, InlineUploadContext, UploadExecution, UploadReservationStore,
        UploadSession,
    },
};
use bytes::Bytes;
use futures::Stream;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct UploadApplicationService {
    authorization: Arc<dyn Authorization>,
    filesystem: Arc<dyn FileSystemResolver>,
    effects: Arc<dyn FileMutationEffects>,
    mutations: Arc<dyn MutationCoordinator>,
    reservations: Arc<dyn UploadReservationStore>,
    execution: Arc<dyn UploadExecution>,
    local_root: PathBuf,
    max_editable_size: u64,
    max_upload_size: u64,
}

impl UploadApplicationService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        authorization: Arc<dyn Authorization>,
        filesystem: Arc<dyn FileSystemResolver>,
        effects: Arc<dyn FileMutationEffects>,
        mutations: Arc<dyn MutationCoordinator>,
        reservations: Arc<dyn UploadReservationStore>,
        execution: Arc<dyn UploadExecution>,
        local_root: PathBuf,
        max_editable_size: u64,
        max_upload_size: u64,
    ) -> Self {
        Self {
            authorization,
            filesystem,
            effects,
            mutations,
            reservations,
            execution,
            local_root,
            max_editable_size,
            max_upload_size,
        }
    }

    pub fn validate_target(
        connection: &ConnectionId,
        dest_path: &str,
    ) -> Result<VfsPath, AppError> {
        Ok(VfsPath::new(connection.as_str(), dest_path)?)
    }

    async fn target_exists(
        provider: &Arc<dyn crate::vfs::FileSystem>,
        target: &VfsPath,
    ) -> Result<bool, AppError> {
        match provider.stat(target).await {
            Ok(_) => Ok(true),
            Err(VfsError::NotFound(_)) => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    pub async fn create_session(
        &self,
        actor: &Actor,
        connection: &ConnectionId,
        target: VfsPath,
        file_name: String,
        total_bytes: Option<u64>,
    ) -> Result<UploadSession, AppError> {
        self.authorization
            .authorize(actor, connection, FileAction::Upload)
            .await?;
        let provider = self.filesystem.resolve(connection).await?;
        let target_exists = Self::target_exists(&provider, &target).await?;
        let target_perms = crate::domain::resolve_destination_permissions_strict(
            &provider,
            &target,
            false,
            PermissionInheritanceMode::InheritExistingOrParent,
        )
        .await?;
        self.ensure_local_capacity(connection)?;

        let lease = self
            .mutations
            .try_acquire(connection, &target.path)
            .await?;
        let prepared = self
            .execution
            .create_inline_job(CreateInlineUploadJob {
                user_id: Some(actor.id.clone()),
                name: file_name.clone(),
                destination_connection_id: connection.to_string(),
                destination_path: target.path.clone(),
                total_bytes,
                inline_threshold: self.max_editable_size,
                target_exists,
                capabilities: provider.capabilities(),
            })
            .await?;
        let session = UploadSession {
            job_id: prepared.job_id.clone(),
            user_id: actor.id.clone(),
            connection_id: connection.to_string(),
            target,
            file_name,
            total_bytes,
            max_upload_bytes: self.max_upload_size,
            target_exists,
            target_perms,
            plan: prepared.plan,
        };
        if let Err(error) = self.reservations.reserve(session.clone(), lease).await {
            self.execution.cancel_inline_job(&prepared.job_id).await;
            return Err(error);
        }
        Ok(session)
    }

    pub async fn execute_session_stream<S>(
        &self,
        actor: &Actor,
        connection: &ConnectionId,
        job_id: &str,
        byte_stream: S,
    ) -> Result<String, AppError>
    where
        S: Stream<Item = Result<Bytes, AppError>> + Send,
    {
        let claimed = self.reservations.claim(job_id, &actor.id).await?;
        let session = claimed.session().clone();

        if session.connection_id != connection.as_str() {
            self.execution.cancel_inline_job(job_id).await;
            return Err(AppError::NotFound(format!(
                "Upload session '{}' not found",
                job_id
            )));
        }

        let session_connection = ConnectionId::new(session.connection_id.clone())
            .map_err(|error| AppError::BadRequest(error.to_string()))?;
        let provider = self.filesystem.resolve(&session_connection).await?;
        let result = self
            .execution
            .execute_inline(
                provider,
                InlineUploadContext {
                    target: session.target.clone(),
                    job_id: session.job_id.clone(),
                    plan: session.plan,
                    total_hint: session.total_bytes,
                    max_bytes: session.max_upload_bytes,
                    target_exists: session.target_exists,
                    target_perms: session.target_perms,
                },
                Box::pin(byte_stream),
            )
            .await;

        drop(claimed);
        self.finish_upload(actor, &session_connection, &session.target, job_id, result)
            .await
    }

    pub async fn execute_inline_stream<S>(
        &self,
        actor: &Actor,
        connection: &ConnectionId,
        target: VfsPath,
        file_name: &str,
        total_hint: Option<u64>,
        byte_stream: S,
    ) -> Result<String, AppError>
    where
        S: Stream<Item = Result<Bytes, AppError>> + Send,
    {
        self.authorization
            .authorize(actor, connection, FileAction::Upload)
            .await?;
        let provider = self.filesystem.resolve(connection).await?;
        let target_exists = Self::target_exists(&provider, &target).await?;
        let target_perms = crate::domain::resolve_destination_permissions_strict(
            &provider,
            &target,
            false,
            PermissionInheritanceMode::InheritExistingOrParent,
        )
        .await?;
        self.ensure_local_capacity(connection)?;

        let _lease = self
            .mutations
            .try_acquire(connection, &target.path)
            .await?;
        let prepared = self
            .execution
            .create_inline_job(CreateInlineUploadJob {
                user_id: Some(actor.id.clone()),
                name: file_name.to_string(),
                destination_connection_id: connection.to_string(),
                destination_path: target.path.clone(),
                total_bytes: total_hint,
                inline_threshold: self.max_editable_size,
                target_exists,
                capabilities: provider.capabilities(),
            })
            .await?;
        let job_id = prepared.job_id;
        let result = self
            .execution
            .execute_inline(
                provider,
                InlineUploadContext {
                    target: target.clone(),
                    job_id: job_id.clone(),
                    plan: prepared.plan,
                    total_hint,
                    max_bytes: self.max_upload_size,
                    target_exists,
                    target_perms,
                },
                Box::pin(byte_stream),
            )
            .await;
        self.finish_upload(actor, connection, &target, &job_id, result)
            .await
    }

    async fn finish_upload(
        &self,
        actor: &Actor,
        connection: &ConnectionId,
        target: &VfsPath,
        job_id: &str,
        result: Result<u64, AppError>,
    ) -> Result<String, AppError> {
        match result {
            Ok(_) => {
                self.effects.invalidate(connection, &target.path).await;
                if let Err(error) = self
                    .effects
                    .file_changed(
                        actor,
                        connection,
                        &target.path,
                        "FILE_UPLOAD",
                        "upload",
                        Some(format!("Uploaded: {} via Transfer {}", target.path, job_id)),
                    )
                    .await
                {
                    self.execution
                        .fail_inline_job(
                            job_id,
                            format!(
                                "upload content committed but durable post-commit effects failed: {}",
                                error
                            ),
                        )
                        .await;
                    return Err(error);
                }
                self.execution.complete_inline_job(job_id).await;
                Ok(target.path.clone())
            }
            Err(error) => {
                if matches!(error, AppError::Cancelled(_)) {
                    self.execution.cancel_inline_job(job_id).await;
                } else {
                    self.execution
                        .fail_inline_job(job_id, error.to_string())
                        .await;
                }
                Err(error)
            }
        }
    }

    fn ensure_local_capacity(&self, connection: &ConnectionId) -> Result<(), AppError> {
        if connection.as_str() == ConnectionId::LOCAL {
            if let Some(free_bytes) = get_available_disk_space(&self.local_root) {
                if free_bytes < 10 * 1024 * 1024 {
                    return Err(AppError::InsufficientStorage(format!(
                        "Local filesystem storage full: only {} MB free",
                        free_bytes / (1024 * 1024)
                    )));
                }
            }
        }
        Ok(())
    }
}

#[cfg(unix)]
fn get_available_disk_space(path: &std::path::Path) -> Option<u64> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let c_path = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) } == 0 {
        Some((stat.f_bavail as u64) * (stat.f_frsize as u64))
    } else {
        None
    }
}

#[cfg(not(unix))]
fn get_available_disk_space(_path: &std::path::Path) -> Option<u64> {
    None
}
