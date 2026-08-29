use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use crate::domain::FileKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SyncStrategy {
    SourceWins,
    DestWins,
    NewestWins,
    #[default]
    KeepBoth,
    Manual,
}

impl std::str::FromStr for SyncStrategy {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "source_wins" => SyncStrategy::SourceWins,
            "dest_wins" => SyncStrategy::DestWins,
            "newest_wins" => SyncStrategy::NewestWins,
            "manual" => SyncStrategy::Manual,
            _ => SyncStrategy::KeepBoth,
        })
    }
}

impl SyncStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            SyncStrategy::SourceWins => "source_wins",
            SyncStrategy::DestWins => "dest_wins",
            SyncStrategy::NewestWins => "newest_wins",
            SyncStrategy::KeepBoth => "keep_both",
            SyncStrategy::Manual => "manual",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SyncStatus {
    Created,
    Scanning,
    Planning,
    Reconciling,
    Executing,
    Verifying,
    Completed,
    Paused,
    Failed,
    Conflict,
}

impl SyncStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SyncStatus::Created => "created",
            SyncStatus::Scanning => "scanning",
            SyncStatus::Planning => "planning",
            SyncStatus::Reconciling => "reconciling",
            SyncStatus::Executing => "executing",
            SyncStatus::Verifying => "verifying",
            SyncStatus::Completed => "completed",
            SyncStatus::Paused => "paused",
            SyncStatus::Failed => "failed",
            SyncStatus::Conflict => "conflict",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct FileManifest {
    pub path: String,
    pub kind: FileKind,
    pub size: u64,
    pub modified_at: Option<DateTime<Utc>>,
    pub content_hash: Option<String>,
    pub etag: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncOpKind {
    Create,
    Update,
    Delete,
    Rename { old_path: String },
    Noop,
    Conflict,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncOperation {
    pub relative_path: String,
    pub kind: SyncOpKind,
    pub source_manifest: Option<FileManifest>,
    pub dest_manifest: Option<FileManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SyncJob {
    pub id: String,
    pub user_id: String,
    pub source_connection_id: String,
    pub source_path: String,
    pub destination_connection_id: String,
    pub destination_path: String,
    pub status: SyncStatus,
    pub strategy: SyncStrategy,
    pub total_files: u64,
    pub synced_files: u64,
    pub conflict_files: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
