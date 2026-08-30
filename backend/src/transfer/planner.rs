use crate::domain::VfsPath;
use crate::transfer::{TransferExecutionMode, TransferJob, TransferStaging, TransferType};
use crate::vfs::FileSystem;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferStrategy {
    /// Instant atomic filesystem rename within the same connection/provider
    NativeRename,
    /// Fast-path zero-egress server-side copy (e.g. S3 CopyObject or local fs hardlink/clone)
    ServerSideCopy,
    /// Asynchronous streaming transfer across different connections or as fallback
    Streaming,
}

pub struct TransferPlanner;

impl TransferPlanner {
    /// Determine the most efficient and semantically correct transfer strategy
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
            // Strict Server-Side Copy: Cloud object storage (S3 CopyObject) to avoid egress
            if caps.server_side_copy && caps.multipart_write {
                return TransferStrategy::ServerSideCopy;
            }
        }

        TransferStrategy::Streaming
    }

    /// Select staging strategy for Upload based on provider capabilities — implementation detail of TransferEngine
    pub fn upload_staging(capabilities: &crate::domain::Capabilities) -> TransferStaging {
        if capabilities.atomic_rename {
            TransferStaging::LocalTemp
        } else if capabilities.atomic_write {
            TransferStaging::ProviderTemp
        } else {
            TransferStaging::None
        }
    }

    /// Select execution mode for Upload based on size & config — small inline, large resumable
    pub fn upload_execution_mode(total_bytes: Option<u64>, inline_threshold: u64) -> TransferExecutionMode {
        match total_bytes {
            Some(n) if n > inline_threshold => TransferExecutionMode::Resumable,
            _ => TransferExecutionMode::Inline,
        }
    }
}
