use crate::db::DbPool;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

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
