use crate::auth::audit::{record_audit_log, AuditLogEntry};
use crate::auth::AuthenticatedUser;
use crate::db::DbPool;
use crate::errors::AppError;

#[derive(Clone)]
pub struct AuditService {
    db: DbPool,
}

impl AuditService {
    pub fn new(db: DbPool) -> Self {
        Self { db }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn record(
        &self,
        user_id: Option<&str>,
        action: &str,
        connection_id: Option<&str>,
        resource_path: Option<&str>,
        status: &str,
        ip_address: Option<&str>,
        details: Option<&str>,
    ) {
        record_audit_log(
            &self.db,
            user_id,
            action,
            connection_id,
            resource_path,
            status,
            ip_address,
            details,
        )
        .await;
    }

    pub async fn list_logs(
        &self,
        user: &AuthenticatedUser,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<AuditLogEntry>, AppError> {
        if !user.is_admin {
            return Err(AppError::Forbidden(
                "Access forbidden: Only administrators can view system audit logs".to_string(),
            ));
        }

        let rows = sqlx::query_as::<_, (String, Option<String>, Option<String>, String, Option<String>, Option<String>, String, Option<String>, Option<String>, String)>(
            "SELECT a.id, a.user_id, u.username, a.action, a.connection_id, a.path, a.status, a.ip_address, a.details, a.created_at \n             FROM audit_logs a\n             LEFT JOIN users u ON a.user_id = u.id\n             ORDER BY a.created_at DESC LIMIT ? OFFSET ?"
        )
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.db)
        .await
        .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

        Ok(rows
            .into_iter()
            .map(
                |(
                    id,
                    user_id,
                    username,
                    action,
                    connection_id,
                    path,
                    status,
                    ip_address,
                    details,
                    created_at,
                )| AuditLogEntry {
                    id,
                    user_id,
                    username,
                    action,
                    connection_id,
                    path,
                    status,
                    ip_address,
                    details,
                    created_at,
                },
            )
            .collect())
    }
}
