use crate::auth::{AuditLogEntry, AuthenticatedUser};
use crate::errors::AppError;
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
    if !user.is_admin {
        return Err(AppError::Forbidden("Only administrators can view audit logs".into()));
    }

    let limit = query.limit.unwrap_or(100).min(500);
    let offset = query.offset.unwrap_or(0);

    let rows: Vec<(
        String,
        Option<String>,
        String,
        Option<String>,
        Option<String>,
        String,
        Option<String>,
        Option<String>,
        String,
    )> = sqlx::query_as(
        "SELECT id, user_id, action, connection_id, path, status, ip_address, details, created_at 
         FROM audit_logs 
         ORDER BY created_at DESC 
         LIMIT ? OFFSET ?",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

    let mut logs = Vec::new();
    for (id, user_id, action, connection_id, path, status, ip_address, details, created_at) in rows {
        logs.push(AuditLogEntry {
            id,
            user_id,
            action,
            connection_id,
            path,
            status,
            ip_address,
            details,
            created_at,
        });
    }

    Ok(Json(logs))
}
