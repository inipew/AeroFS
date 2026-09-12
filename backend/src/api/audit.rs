use crate::api::extractors::Query;
use crate::auth::audit::AuditLogEntry;
use crate::auth::AuthenticatedUser;
use crate::errors::{AppError, ErrorResponse};
use crate::state::AuditState;
use axum::{extract::State, response::IntoResponse, Json};
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct AuditLogQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Get system audit logs (Admin only)
#[utoipa::path(
    get,
    path = "/api/v1/audit-logs",
    params(AuditLogQuery),
    responses(
        (status = 200, description = "List of audit logs", body = Vec<AuditLogEntry>),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(("CookieAuth" = []), ("BearerAuth" = [])),
    tag = "audit"
)]
pub async fn list_audit_logs(
    State(state): State<AuditState>,
    user: AuthenticatedUser,
    Query(query): Query<AuditLogQuery>,
) -> Result<impl IntoResponse, AppError> {
    let limit = query.limit.unwrap_or(100).clamp(1, 500) as usize;
    let offset = query.offset.unwrap_or(0).max(0) as usize;
    Ok(Json(state.service.list_logs(&user, limit, offset).await?))
}
