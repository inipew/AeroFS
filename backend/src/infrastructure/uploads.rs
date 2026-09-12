use crate::domain::CommitSemantics;
use crate::errors::AppError;
use crate::ports::upload::{
    CreateInlineUploadJob, InlineUploadContext, PreparedInlineUpload, UploadByteStream,
    UploadExecution, UploadPlan, UploadStaging,
};
use crate::transfer::{
    executor, planner::TransferPlanner, planner::UploadConstraints, TransferExecutionMode,
    TransferManager, TransferPlan, TransferStaging,
};
use crate::vfs::FileSystem;
use async_trait::async_trait;
use std::sync::Arc;

#[derive(Clone)]
pub struct TransferUploadExecution {
    manager: TransferManager,
}

impl TransferUploadExecution {
    pub fn new(manager: TransferManager) -> Self {
        Self { manager }
    }

    fn to_port_plan(plan: &TransferPlan) -> UploadPlan {
        UploadPlan {
            staging: match plan.staging {
                TransferStaging::None => UploadStaging::None,
                TransferStaging::LocalTemp => UploadStaging::LocalTemp,
                TransferStaging::ProviderTemp => UploadStaging::ProviderTemp,
            },
            commit: plan.commit,
        }
    }

    fn to_transfer_plan(plan: UploadPlan) -> TransferPlan {
        TransferPlan {
            execution_mode: TransferExecutionMode::Inline,
            staging: match plan.staging {
                UploadStaging::None => TransferStaging::None,
                UploadStaging::LocalTemp => TransferStaging::LocalTemp,
                UploadStaging::ProviderTemp => TransferStaging::ProviderTemp,
            },
            commit: match plan.commit {
                CommitSemantics::Direct => CommitSemantics::Direct,
                CommitSemantics::AtomicRename => CommitSemantics::AtomicRename,
                CommitSemantics::ProviderAtomic => CommitSemantics::ProviderAtomic,
            },
        }
    }
}

#[async_trait]
impl UploadExecution for TransferUploadExecution {
    async fn create_inline_job(
        &self,
        request: CreateInlineUploadJob,
    ) -> Result<PreparedInlineUpload, AppError> {
        let plan = TransferPlanner::plan_upload(
            &request.capabilities,
            UploadConstraints::inline(request.total_bytes),
            request.inline_threshold,
            request.target_exists,
        );
        let job = self
            .manager
            .create_inline_upload_job_with_plan(
                request.user_id,
                request.name,
                request.destination_connection_id,
                request.destination_path,
                request.total_bytes,
                plan.clone(),
            )
            .await;
        Ok(PreparedInlineUpload {
            job_id: job.id,
            plan: Self::to_port_plan(&plan),
        })
    }

    async fn execute_inline(
        &self,
        provider: Arc<dyn FileSystem>,
        context: InlineUploadContext,
        stream: UploadByteStream,
    ) -> Result<u64, AppError> {
        let cancel_token = match self.manager.cancel_token(&context.job_id) {
            Some(token) => token,
            None => {
                self.manager
                    .fail_inline_job(
                        &context.job_id,
                        "transfer cancellation token missing".into(),
                    )
                    .await;
                return Err(AppError::Internal(anyhow::anyhow!(
                    "transfer cancellation token missing"
                )));
            }
        };

        executor::execute_inline_upload_stream(
            &self.manager,
            provider,
            executor::InlineUploadContext {
                target: context.target,
                job_id: context.job_id,
                plan: Self::to_transfer_plan(context.plan),
                total_hint: context.total_hint,
                max_bytes: context.max_bytes,
                target_exists: context.target_exists,
                target_perms: context.target_perms,
            },
            cancel_token,
            stream,
        )
        .await
    }

    async fn complete_inline_job(&self, job_id: &str) {
        self.manager.complete_inline_job(job_id, None).await;
    }

    async fn fail_inline_job(&self, job_id: &str, error: String) {
        self.manager.fail_inline_job(job_id, error).await;
    }

    async fn cancel_inline_job(&self, job_id: &str) {
        self.manager.cancel_inline_job(job_id).await;
    }
}
