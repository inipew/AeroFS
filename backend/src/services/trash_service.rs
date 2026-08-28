use crate::auth::audit::record_audit_log;
use crate::auth::permissions::{check_permission, PermissionAction};
use crate::auth::AuthenticatedUser;
use crate::domain::{FileKind, VfsPath};
use crate::errors::AppError;
use crate::state::AppState;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct TrashItem {
    pub id: String,
    pub connection_id: String,
    pub original_path: String,
    pub item_name: String,
    pub is_directory: bool,
    pub size: Option<i64>,
    pub deleted_at: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MoveToTrashRequest {
    pub connection_id: String,
    pub paths: Vec<String>,
}

type TrashDbRow = (String, String, String, String, i64, Option<i64>, String);

pub struct TrashService;

impl TrashService {
    pub async fn list_trash(
        state: &AppState,
        _user: &AuthenticatedUser,
    ) -> Result<Vec<TrashItem>, AppError> {
        let rows: Vec<TrashDbRow> = sqlx::query_as(
            "SELECT id, connection_id, original_path, item_name, is_directory, size, deleted_at FROM trash_items ORDER BY deleted_at DESC"
        )
        .fetch_all(&state.db)
        .await
        .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

        let items: Vec<TrashItem> = rows
            .into_iter()
            .map(
                |(id, connection_id, original_path, item_name, is_dir, size, deleted_at)| {
                    TrashItem {
                        id,
                        connection_id,
                        original_path,
                        item_name,
                        is_directory: is_dir != 0,
                        size,
                        deleted_at,
                    }
                },
            )
            .collect();

        Ok(items)
    }

    pub async fn move_to_trash(
        state: &AppState,
        user: &AuthenticatedUser,
        payload: MoveToTrashRequest,
    ) -> Result<usize, AppError> {
        check_permission(
            &state.db,
            user,
            &payload.connection_id,
            PermissionAction::Delete,
        )
        .await?;

        let provider = state
            .registry
            .get(&payload.connection_id)
            .await
            .ok_or_else(|| AppError::NotFound("Storage connection not found".into()))?;

        let now_str = Utc::now().to_rfc3339();
        let trash_dir_vfs = VfsPath::new(&payload.connection_id, "/.trash")?;
        let _ = provider.create_dir(&trash_dir_vfs).await;

        let mut moved_count = 0;

        for path_str in &payload.paths {
            let vfs_path = match VfsPath::new(&payload.connection_id, path_str) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if let Ok(meta) = provider.stat(&vfs_path).await {
                let item_id = Uuid::new_v4().to_string();
                let trash_filename = format!("/.trash/{}_{}", &item_id[..8], meta.name);
                let dest_vfs = match VfsPath::new(&payload.connection_id, &trash_filename) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                if provider.rename(&vfs_path, &dest_vfs).await.is_ok() {
                    let is_dir = if meta.kind == FileKind::Directory {
                        1
                    } else {
                        0
                    };
                    let _ = sqlx::query(
                        "INSERT INTO trash_items (id, connection_id, original_path, trash_path, item_name, is_directory, size, deleted_at, deleted_by)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
                    )
                    .bind(&item_id)
                    .bind(&payload.connection_id)
                    .bind(path_str)
                    .bind(&trash_filename)
                    .bind(&meta.name)
                    .bind(is_dir)
                    .bind(meta.size as i64)
                    .bind(&now_str)
                    .bind(&user.username)
                    .execute(&state.db)
                    .await;

                    record_audit_log(
                        &state.db,
                        Some(&user.id),
                        "TRASH_MOVE",
                        Some(&payload.connection_id),
                        Some(path_str),
                        "SUCCESS",
                        None,
                        Some(&format!("Moved {} to trash", path_str)),
                    )
                    .await;

                    moved_count += 1;
                }
            }
        }

        Ok(moved_count)
    }

    pub async fn restore_item(
        state: &AppState,
        user: &AuthenticatedUser,
        trash_id: &str,
    ) -> Result<(), AppError> {
        let row: Option<(String, String, String)> = sqlx::query_as(
            "SELECT connection_id, original_path, trash_path FROM trash_items WHERE id = ?",
        )
        .bind(trash_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

        let (connection_id, orig_path, trash_path) =
            row.ok_or_else(|| AppError::NotFound("Trash item not found".into()))?;

        check_permission(&state.db, user, &connection_id, PermissionAction::Create).await?;
        check_permission(&state.db, user, &connection_id, PermissionAction::Write).await?;

        let provider = state
            .registry
            .get(&connection_id)
            .await
            .ok_or_else(|| AppError::NotFound("Storage connection not found".into()))?;

        let trash_vfs = VfsPath::new(&connection_id, &trash_path)?;
        let orig_vfs = VfsPath::new(&connection_id, &orig_path)?;

        provider.rename(&trash_vfs, &orig_vfs).await?;

        sqlx::query("DELETE FROM trash_items WHERE id = ?")
            .bind(trash_id)
            .execute(&state.db)
            .await
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

        record_audit_log(
            &state.db,
            Some(&user.id),
            "TRASH_RESTORE",
            Some(&connection_id),
            Some(&orig_path),
            "SUCCESS",
            None,
            Some(&format!("Restored {} from trash", orig_path)),
        )
        .await;

        Ok(())
    }

    pub async fn empty_trash(
        state: &AppState,
        user: &AuthenticatedUser,
    ) -> Result<usize, AppError> {
        let rows: Vec<(String, String, String)> =
            sqlx::query_as("SELECT id, connection_id, trash_path FROM trash_items")
                .fetch_all(&state.db)
                .await
                .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

        let mut deleted_count = 0;
        for (id, conn_id, trash_path) in rows {
            if check_permission(&state.db, user, &conn_id, PermissionAction::Delete)
                .await
                .is_err()
            {
                continue;
            }

            if let Some(provider) = state.registry.get(&conn_id).await {
                if let Ok(trash_vfs) = VfsPath::new(&conn_id, &trash_path) {
                    let _ = provider.delete(&trash_vfs).await;
                }
            }
            let _ = sqlx::query("DELETE FROM trash_items WHERE id = ?")
                .bind(&id)
                .execute(&state.db)
                .await;
            deleted_count += 1;
        }

        Ok(deleted_count)
    }
}
