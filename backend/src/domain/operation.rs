use crate::domain::path::VfsPath;
use crate::domain::policy::PermissionInheritanceMode;
use crate::filesystem::archive::ArchiveOverwriteMode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum OperationStatus {
    Requested,
    Validated,
    Executing,
    Verifying,
    Committed,
    Completed,
    Partial,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum OperationIntentType {
    Copy,
    Move,
    Delete,
    Chmod,
    Compress,
    Extract,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FailureStrategy {
    FailFast,
    #[default]
    ContinueOnFailure,
    BestEffort,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OperationPlan {
    pub id: String,
    pub intent_type: OperationIntentType,
    pub source_connection_id: String,
    pub source_paths: Vec<VfsPath>,
    pub destination_connection_id: Option<String>,
    pub destination_path: Option<VfsPath>,
    pub failure_strategy: FailureStrategy,
    pub permission_mode: PermissionInheritanceMode,
    pub overwrite_mode: Option<ArchiveOverwriteMode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OperationExecutionResult {
    pub plan_id: String,
    pub status: OperationStatus,
    pub total_items: usize,
    pub succeeded_items: Vec<String>,
    pub failed_items: Vec<(String, String)>,
    pub skipped_items: Vec<String>,
}

impl OperationExecutionResult {
    pub fn new(plan_id: String, total_items: usize) -> Self {
        Self {
            plan_id,
            status: OperationStatus::Executing,
            total_items,
            succeeded_items: Vec::new(),
            failed_items: Vec::new(),
            skipped_items: Vec::new(),
        }
    }

    pub fn is_success(&self) -> bool {
        self.failed_items.is_empty()
    }

    pub fn finalize(&mut self) {
        if self.failed_items.is_empty() {
            self.status = OperationStatus::Completed;
        } else if !self.succeeded_items.is_empty() {
            self.status = OperationStatus::Partial;
        } else {
            self.status = OperationStatus::Failed;
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OperationRecord {
    pub id: String,
    pub idempotency_key: Option<String>,
    pub operation_type: String,
    pub user_id: Option<String>,
    pub connection_id: String,
    pub source_path: Option<String>,
    pub destination_path: Option<String>,
    pub status: OperationStatus,
    pub completed_items: usize,
    pub failed_items: usize,
    pub total_items: usize,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl OperationRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: String,
        idempotency_key: Option<String>,
        operation_type: String,
        user_id: Option<String>,
        connection_id: String,
        source_path: Option<String>,
        destination_path: Option<String>,
        total_items: usize,
    ) -> Self {
        let now = Utc::now();
        Self {
            id,
            idempotency_key,
            operation_type,
            user_id,
            connection_id,
            source_path,
            destination_path,
            status: OperationStatus::Requested,
            completed_items: 0,
            failed_items: 0,
            total_items,
            error_message: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn set_status(&mut self, status: OperationStatus) {
        self.status = status;
        self.updated_at = Utc::now();
    }
}
