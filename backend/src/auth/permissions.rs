use crate::auth::UserInfo;
use crate::db::DbPool;
use crate::errors::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionAction {
    Read,
    Write,
    Create,
    Delete,
    Rename,
    Upload,
    Download,
}

impl PermissionAction {
    pub fn is_mutating(&self) -> bool {
        matches!(
            self,
            PermissionAction::Write
                | PermissionAction::Create
                | PermissionAction::Delete
                | PermissionAction::Rename
                | PermissionAction::Upload
        )
    }
}

/// Verify if a user is authorized to perform a specific action on a connection
pub async fn check_permission(
    db: &DbPool,
    user: &UserInfo,
    connection_id: &str,
    action: PermissionAction,
) -> Result<(), AppError> {
    // 1. Centralized connection read_only policy enforcement
    if action.is_mutating() && connection_id != "local" {
        let conn_row: Option<(i64,)> = sqlx::query_as(
            "SELECT read_only FROM connections WHERE id = ?",
        )
        .bind(connection_id)
        .fetch_optional(db)
        .await
        .map_err(|e| anyhow::anyhow!("Database error checking connection status: {}", e))?;

        if let Some((read_only,)) = conn_row {
            if read_only != 0 {
                return Err(AppError::Forbidden(format!(
                    "Storage connection '{}' is configured as read-only. Mutation {:?} is rejected.",
                    connection_id, action
                )));
            }
        }
    }

    // Administrator has full permissions on all non-read-only connections
    if user.is_admin {
        return Ok(());
    }

    // 2. Query permissions table
    let row: Option<(i64, i64, i64, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT can_read, can_write, can_create, can_delete, can_rename, can_upload, can_download
         FROM permissions
         WHERE user_id = ? AND connection_id = ?",
    )
    .bind(&user.id)
    .bind(connection_id)
    .fetch_optional(db)
    .await
    .map_err(|e| anyhow::anyhow!("Database error checking permissions: {}", e))?;

    let (can_read, can_write, can_create, can_delete, can_rename, can_upload, can_download) =
        match row {
            Some(perms) => perms,
            None => {
                // Default fallback: allow local read/write if not explicitly restricted, or deny remote
                if connection_id == "local" {
                    (1, 1, 1, 1, 1, 1, 1)
                } else {
                    return Err(AppError::Forbidden(format!(
                        "User '{}' has no permission assigned for storage connection '{}'",
                        user.username, connection_id
                    )));
                }
            }
        };

    let allowed = match action {
        PermissionAction::Read => can_read != 0,
        PermissionAction::Write => can_write != 0,
        PermissionAction::Create => can_create != 0,
        PermissionAction::Delete => can_delete != 0,
        PermissionAction::Rename => can_rename != 0,
        PermissionAction::Upload => can_upload != 0,
        PermissionAction::Download => can_download != 0,
    };

    if !allowed {
        return Err(AppError::Forbidden(format!(
            "User '{}' is not authorized to perform {:?} on connection '{}'",
            user.username, action, connection_id
        )));
    }

    Ok(())
}
