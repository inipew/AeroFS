use crate::auth::audit::{record_audit_log, AuditLogEntry};
use crate::auth::AuthenticatedUser;
use crate::db::DbPool;
use crate::errors::AppError;

pub struct AuditService;

impl AuditService {
    #[allow(clippy::too_many_arguments)]
    pub async fn record(
        db: &DbPool,
        user_id: Option<&str>,
        action: &str,
        connection_id: Option<&str>,
        resource_path: Option<&str>,
        status: &str,
        ip_address: Option<&str>,
        details: Option<&str>,
    ) {
        record_audit_log(
            db,
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
        db: &DbPool,
        user: &AuthenticatedUser,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<AuditLogEntry>, AppError> {
        if !user.is_admin {
            return Err(AppError::Forbidden(
                "Access forbidden: Only administrators can view system audit logs".to_string(),
            ));
        }

        let rows = sqlx::query_as::<_, (String, Option<String>, String, Option<String>, Option<String>, String, Option<String>, Option<String>, String)>(
            "SELECT id, user_id, action, connection_id, path, status, ip_address, details, created_at 
             FROM audit_logs ORDER BY created_at DESC LIMIT ? OFFSET ?"
        )
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(db)
        .await
        .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

        let entries = rows
            .into_iter()
            .map(
                |(
                    id,
                    user_id,
                    action,
                    connection_id,
                    path,
                    status,
                    ip_address,
                    details,
                    created_at,
                )| {
                    AuditLogEntry {
                        id,
                        user_id,
                        action,
                        connection_id,
                        path,
                        status,
                        ip_address,
                        details,
                        created_at,
                    }
                },
            )
            .collect();

        Ok(entries)
    }
}
