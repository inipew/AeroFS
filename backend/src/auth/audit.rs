use crate::db::DbPool;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    FileCreate,
    FileDelete,
    FileMove,
    FileCopy,
    FileRename,
    FileDownload,
    FileUpload,
    FileChmod,
    PresignDownload,
    PresignUpload,
    AuthLogin,
    AuthLogout,
    ShareCreate,
    ShareDelete,
    TransferCreate,
    TransferCancel,
    TransferRetry,
    ConnectionCreate,
    ConnectionUpdate,
    ConnectionDelete,
}

impl AuditAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FileCreate => "file.create",
            Self::FileDelete => "file.delete",
            Self::FileMove => "file.move",
            Self::FileCopy => "file.copy",
            Self::FileRename => "file.rename",
            Self::FileDownload => "file.download",
            Self::FileUpload => "file.upload",
            Self::FileChmod => "file.chmod",
            Self::PresignDownload => "file.presign_download",
            Self::PresignUpload => "file.presign_upload",
            Self::AuthLogin => "auth.login",
            Self::AuthLogout => "auth.logout",
            Self::ShareCreate => "share.create",
            Self::ShareDelete => "share.delete",
            Self::TransferCreate => "transfer.create",
            Self::TransferCancel => "transfer.cancel",
            Self::TransferRetry => "transfer.retry",
            Self::ConnectionCreate => "connection.create",
            Self::ConnectionUpdate => "connection.update",
            Self::ConnectionDelete => "connection.delete",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AuditOutcome {
    Success,
    Failure,
}

impl AuditOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AuditLogEntry {
    pub id: String,
    pub user_id: Option<String>,
    pub action: String,
    pub connection_id: Option<String>,
    pub path: Option<String>,
    pub status: String,
    pub ip_address: Option<String>,
    pub details: Option<String>,
    pub created_at: String,
}

pub async fn record_audit_log_typed(
    db: &DbPool,
    user_id: Option<&str>,
    action: AuditAction,
    connection_id: Option<&str>,
    path: Option<&str>,
    outcome: AuditOutcome,
    ip_address: Option<&str>,
    details: Option<&str>,
) {
    record_audit_log(
        db,
        user_id,
        action.as_str(),
        connection_id,
        path,
        outcome.as_str(),
        ip_address,
        details,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn record_audit_log(
    db: &DbPool,
    user_id: Option<&str>,
    action: &str,
    connection_id: Option<&str>,
    path: Option<&str>,
    status: &str,
    ip_address: Option<&str>,
    details: Option<&str>,
) {
    let id = format!("log_{}", &Uuid::new_v4().to_string()[..12]);
    let now = Utc::now().to_rfc3339();

    let _ = sqlx::query(
        "INSERT INTO audit_logs (id, user_id, action, connection_id, path, status, ip_address, details, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(user_id)
    .bind(action)
    .bind(connection_id)
    .bind(path)
    .bind(status)
    .bind(ip_address)
    .bind(details)
    .bind(&now)
    .execute(db)
    .await;
}
