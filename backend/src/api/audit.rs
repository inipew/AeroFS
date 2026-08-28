use crate::auth::AuthenticatedUser;
use crate::errors::AppError;
use crate::services::audit_service::AuditService;
use crate::state::AppState;
use axum::{
    extract::{Query, State},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use utoipa::IntoParams;

#[derive(Debug, Deserialize, IntoParams)]
pub struct AuditLogQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Get system audit logs (Admin only)
pub async fn list_audit_logs(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Query(query): Query<AuditLogQuery>,
) -> Result<impl IntoResponse, AppError> {
    let limit = query.limit.unwrap_or(100).clamp(1, 500) as usize;
    let offset = query.offset.unwrap_or(0).max(0) as usize;

    let logs = AuditService::list_logs(&state.db, &user, limit, offset).await?;
    Ok(Json(logs))
}
