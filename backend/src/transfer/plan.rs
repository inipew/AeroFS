use crate::domain::{CommitSemantics, VfsPath};
use crate::transfer::{TransferExecutionMode, TransferStaging};

/// Single source of truth for upload/transfer execution decisions.
/// Provider capabilities → Planner → Plan → Engine (engine must not re-decide).
#[derive(Debug, Clone)]
pub struct TransferPlan {
    pub execution_mode: TransferExecutionMode,
    pub staging: TransferStaging,
    pub commit: CommitSemantics,
    /// Whether a staging temp file must be used (derived from commit).
    pub use_staging_file: bool,
}

impl TransferPlan {
    /// Build the canonical staging path for a target path + job id.
    /// Unified naming: `.<filename>.aerofs-part-<job_id>` in same parent dir.
    /// This replaces the dual naming (`*.aerofs.part` in upload vs `*.aerofs-part-*` in engine).
    pub fn staging_path(&self, target: &VfsPath, job_id: &str) -> Option<VfsPath> {
        if !self.use_staging_file {
            return None;
        }
        let parent = std::path::Path::new(&target.path)
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("");
        let filename = std::path::Path::new(&target.path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file");
        let staging_str = if parent.is_empty() || parent == "/" {
            format!("/.{}.aerofs-part-{}", filename, job_id)
        } else {
            format!(
                "{}/.{}.aerofs-part-{}",
                parent.trim_end_matches('/'),
                filename,
                job_id
            )
        };
        VfsPath::new(&target.connection_id, staging_str).ok()
    }

    /// Convenience: is this plan using any staging at all?
    pub fn has_staging(&self) -> bool {
        self.staging != TransferStaging::None
    }
}

/// Typed wrapper for staging path — prevents raw string formatting scattered across codebase.
#[derive(Debug, Clone)]
pub struct StagingPath(pub VfsPath);

impl StagingPath {
    pub fn new(target: &VfsPath, job_id: &str) -> Self {
        // Use same canonical format as TransferPlan::staging_path
        let plan = TransferPlan {
            execution_mode: TransferExecutionMode::Inline,
            staging: TransferStaging::LocalTemp,
            commit: CommitSemantics::AtomicRename,
            use_staging_file: true,
        };
        let vfs = plan
            .staging_path(target, job_id)
            .unwrap_or_else(|| target.clone());
        Self(vfs)
    }

    pub fn as_vfs(&self) -> &VfsPath {
        &self.0
    }

    pub fn into_vfs(self) -> VfsPath {
        self.0
    }
}
