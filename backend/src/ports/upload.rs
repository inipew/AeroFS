use crate::domain::{Capabilities, CommitSemantics, VfsPath};
use crate::errors::AppError;
use crate::ports::mutation::MutationLease;
use crate::vfs::FileSystem;
use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;
use std::pin::Pin;
use std::sync::Arc;

pub type UploadByteStream<'a> =
    Pin<Box<dyn Stream<Item = Result<Bytes, AppError>> + Send + 'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadStaging {
    None,
    LocalTemp,
    ProviderTemp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UploadPlan {
    pub staging: UploadStaging,
    pub commit: CommitSemantics,
}

#[derive(Clone)]
pub struct UploadSession {
    pub job_id: String,
    pub user_id: String,
    pub connection_id: String,
    pub target: VfsPath,
    pub file_name: String,
    pub total_bytes: Option<u64>,
    pub max_upload_bytes: u64,
    pub target_exists: bool,
    pub target_perms: Option<String>,
    pub plan: UploadPlan,
    /// Exact provider generation used for admission, capability planning, and
    /// permission resolution. Keeping it pinned prevents a later connection
    /// reconfiguration from executing an admitted session against a different
    /// provider with stale admission facts.
    pub provider: Arc<dyn FileSystem>,
}

impl std::fmt::Debug for UploadSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UploadSession")
            .field("job_id", &self.job_id)
            .field("user_id", &self.user_id)
            .field("connection_id", &self.connection_id)
            .field("target", &self.target)
            .field("file_name", &self.file_name)
            .field("total_bytes", &self.total_bytes)
            .field("max_upload_bytes", &self.max_upload_bytes)
            .field("target_exists", &self.target_exists)
            .field("target_perms", &self.target_perms)
            .field("plan", &self.plan)
            .field("provider", &"<pinned FileSystem>")
            .finish()
    }
}

pub trait UploadClaim: Send {
    fn session(&self) -> &UploadSession;
}

#[async_trait]
pub trait UploadReservationStore: Send + Sync {
    async fn reserve(
        &self,
        session: UploadSession,
        lease: Box<dyn MutationLease>,
    ) -> Result<(), AppError>;

    async fn claim(
        &self,
        job_id: &str,
        user_id: &str,
    ) -> Result<Box<dyn UploadClaim>, AppError>;

    async fn release(&self, job_id: &str);
}

#[derive(Debug, Clone)]
pub struct CreateInlineUploadJob {
    pub user_id: Option<String>,
    pub name: String,
    pub destination_connection_id: String,
    pub destination_path: String,
    pub total_bytes: Option<u64>,
    pub inline_threshold: u64,
    pub target_exists: bool,
    pub capabilities: Capabilities,
}

#[derive(Debug, Clone)]
pub struct PreparedInlineUpload {
    pub job_id: String,
    pub plan: UploadPlan,
}

#[derive(Debug, Clone)]
pub struct InlineUploadContext {
    pub target: VfsPath,
    pub job_id: String,
    pub plan: UploadPlan,
    pub total_hint: Option<u64>,
    pub max_bytes: u64,
    pub target_exists: bool,
    pub target_perms: Option<String>,
}

#[async_trait]
pub trait UploadExecution: Send + Sync {
    async fn create_inline_job(
        &self,
        request: CreateInlineUploadJob,
    ) -> Result<PreparedInlineUpload, AppError>;

    async fn execute_inline(
        &self,
        provider: Arc<dyn FileSystem>,
        context: InlineUploadContext,
        stream: UploadByteStream<'_>,
    ) -> Result<u64, AppError>;

    async fn complete_inline_job(&self, job_id: &str);
    async fn fail_inline_job(&self, job_id: &str, error: String);
    async fn cancel_inline_job(&self, job_id: &str);
}
