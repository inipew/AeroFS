use crate::domain::{CommitSemantics, VfsPath};
use crate::transfer::{TransferExecutionMode, TransferStaging};

/// Single source of truth for upload/transfer execution decisions.
/// Provider capabilities → Planner → Plan → Engine (engine must not re-decide).
#[derive(Debug, Clone)]
pub struct TransferPlan {
    pub execution_mode: TransferExecutionMode,
    pub staging: TransferStaging,
    pub commit: CommitSemantics,
}

impl TransferPlan {
    /// Whether this plan requires a staging temp file.
    /// Per agreed semantics: valid iff `staging != None` — combination
    /// validity is guarded by planner/constructor, not hidden in predicate.
    pub fn uses_staging(&self) -> bool {
        self.staging != TransferStaging::None
    }

    /// Legacy alias — prefer `uses_staging()`.
    pub fn has_staging(&self) -> bool {
        self.uses_staging()
    }

    /// Build the canonical staging path for a target path + job id.
    /// Unified naming: `.<filename>.aerofs-part-<job_id>` in same parent dir.
    pub fn staging_path(&self, target: &VfsPath, job_id: &str) -> Option<VfsPath> {
        if !self.uses_staging() {
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
}
