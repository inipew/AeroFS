use crate::domain::Capabilities;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommitSemantics {
    /// Atomic promotion via staging file and rename (Local / POSIX filesystems)
    AtomicRename,
    /// Atomic object put / multipart commit (Cloud object storage, S3, MinIO)
    AtomicObjectPut,
    /// Direct stream to target path (guarded by preflight checks)
    DirectWrite,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteStrategy {
    pub semantics: CommitSemantics,
    pub staging_suffix: Option<String>,
    pub safe_overwrite: bool,
}

impl WriteStrategy {
    pub fn select(capabilities: &Capabilities, target_exists: bool) -> Self {
        if capabilities.atomic_rename {
            Self {
                semantics: CommitSemantics::AtomicRename,
                staging_suffix: Some(".aerofs.part".to_string()),
                safe_overwrite: true,
            }
        } else if capabilities.atomic_write {
            Self {
                semantics: CommitSemantics::AtomicObjectPut,
                staging_suffix: None,
                safe_overwrite: true,
            }
        } else {
            Self {
                semantics: CommitSemantics::DirectWrite,
                staging_suffix: None,
                safe_overwrite: !target_exists,
            }
        }
    }
}
