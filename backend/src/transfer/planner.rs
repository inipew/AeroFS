use crate::domain::VfsPath;
use crate::domain::{Capabilities, CommitSemantics, WriteStrategy};
use crate::transfer::plan::TransferPlan;
use crate::transfer::{TransferExecutionMode, TransferJob, TransferStaging, TransferType};
use crate::vfs::FileSystem;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferStrategy {
    /// Instant atomic filesystem rename within the same connection/provider
    NativeRename,
    /// Fast-path zero-egress server-side copy (for object stores such as S3 CopyObject)
    ServerSideCopy,
    /// Asynchronous streaming transfer across different connections or when the engine
    /// must retain granular cancellation/progress control.
    Streaming,
}

pub struct TransferPlanner;

impl TransferPlanner {
    /// Determine the most efficient and semantically correct transfer strategy.
    ///
    /// Native same-filesystem copy is deliberately kept on the streaming path. A provider
    /// copy call can be one long, non-yielding operation, which prevents the transfer engine
    /// from honoring CancellationToken promptly. Object stores without atomic rename keep
    /// the zero-egress server-side-copy optimization.
    pub fn plan_transfer(
        job: &TransferJob,
        src_fs: &Arc<dyn FileSystem>,
        _dst_fs: &Arc<dyn FileSystem>,
        _src_vfs: &VfsPath,
        _dst_vfs: &VfsPath,
    ) -> TransferStrategy {
        let is_same_connection = job.source_connection_id == job.destination_connection_id;

        if is_same_connection {
            let caps = src_fs.capabilities();
            if job.transfer_type == TransferType::Move && (caps.rename || caps.atomic_rename) {
                return TransferStrategy::NativeRename;
            }

            // Strict object-store fast path: retain engine-controlled streaming for
            // local/rename-capable filesystems so cancellation remains responsive.
            if caps.server_side_copy && caps.multipart_write && !caps.atomic_rename {
                return TransferStrategy::ServerSideCopy;
            }
        }

        TransferStrategy::Streaming
    }

    /// Select staging strategy for Upload based on provider capabilities — implementation detail of TransferEngine
    fn upload_staging(capabilities: &crate::domain::Capabilities) -> TransferStaging {
        if capabilities.atomic_rename {
            TransferStaging::LocalTemp
        } else if capabilities.atomic_write {
            TransferStaging::ProviderTemp
        } else {
            TransferStaging::None
        }
    }

    /// Select execution mode for Upload based on size & config — small inline, large resumable
    fn upload_execution_mode(
        total_bytes: Option<u64>,
        inline_threshold: u64,
    ) -> TransferExecutionMode {
        match total_bytes {
            Some(n) if n > inline_threshold => TransferExecutionMode::Resumable,
            _ => TransferExecutionMode::Inline,
        }
    }
}

/// Upload constraints — transport capability for planner (honest domain model).
/// `supports_resume` = false for current HTTP multipart (stream ephemeral).
/// Resumable is reserved for future chunked/resumable endpoint.
#[derive(Debug, Clone, Copy)]
pub struct UploadConstraints {
    pub total_hint: Option<u64>,
    pub supports_resume: bool,
}

impl UploadConstraints {
    pub fn inline(total_hint: Option<u64>) -> Self {
        Self {
            total_hint,
            supports_resume: false,
        }
    }
}

impl TransferPlanner {
    /// Unified planner: capabilities + constraints → TransferPlan (single source of truth).
    /// Replaces separate calls to `upload_staging` + `upload_execution_mode` + `WriteStrategy::select`.
    /// Engine must execute this plan without re-deciding via capabilities.
    pub fn plan_upload(
        capabilities: &Capabilities,
        constraints: UploadConstraints,
        inline_threshold: u64,
        target_exists: bool,
    ) -> TransferPlan {
        let staging = Self::upload_staging(capabilities);
        // Honest execution mode: only Resumable if transport supports it AND size exceeds threshold.
        let execution_mode = if constraints.supports_resume {
            Self::upload_execution_mode(constraints.total_hint, inline_threshold)
        } else {
            TransferExecutionMode::Inline
        };
        let write_strategy = WriteStrategy::select(capabilities, target_exists);
        // Validity of (staging, commit) is guarded here; uses_staging() is staging != None.
        // If commit is not AtomicRename, staging file is not required — reconcile.
        let effective_staging = if write_strategy.semantics != CommitSemantics::AtomicRename
            && staging == TransferStaging::LocalTemp
        {
            TransferStaging::None
        } else {
            staging
        };
        TransferPlan {
            execution_mode,
            staging: effective_staging,
            commit: write_strategy.semantics,
        }
    }
}
