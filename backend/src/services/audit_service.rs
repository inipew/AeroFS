use crate::auth::audit::AuditLogEntry;
use crate::auth::AuthenticatedUser;
use crate::errors::AppError;
use crate::ports::audit::{AuditRecord, AuditRepository};
use std::sync::Arc;

#[derive(Clone)]
pub struct AuditService {
    repository: Arc<dyn AuditRepository>,
}

impl AuditService {
    pub fn new(repository: Arc<dyn AuditRepository>) -> Self {
        Self { repository }
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
    ) -> Result<(), AppError> {
        self.repository
            .record(AuditRecord {
                user_id,
                action,
                connection_id,
                resource_path,
                status,
                ip_address,
                details,
            })
            .await
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

        self.repository.list(limit, offset).await
    }
}
