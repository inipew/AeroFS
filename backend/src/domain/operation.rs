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
