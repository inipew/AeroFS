use crate::auth::AuthenticatedUser;
use crate::errors::AppError;
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

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

/// List all items in trash
pub async fn list_trash(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    let rows: Vec<(
        String,
        String,
        String,
        String,
        i64,
        Option<i64>,
        String,
    )> = sqlx::query_as(
        "SELECT id, connection_id, original_path, item_name, is_directory, size, deleted_at FROM trash_items ORDER BY deleted_at DESC"
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

    let items: Vec<TrashItem> = rows
        .into_iter()
        .map(|(id, connection_id, original_path, item_name, is_dir, size, deleted_at)| {
            TrashItem {
                id,
                connection_id,
                original_path,
                item_name,
                is_directory: is_dir != 0,
                size,
                deleted_at,
            }
        })
        .collect();

    Ok(Json(items))
}

/// Move one or more items to trash (soft delete)
pub async fn move_to_trash(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<MoveToTrashRequest>,
) -> Result<impl IntoResponse, AppError> {
    let provider = state
        .get_provider(&payload.connection_id)
        .await
        .ok_or_else(|| AppError::NotFound("Storage connection not found".into()))?;

    let now_str = chrono::Utc::now().to_rfc3339();
    let trash_dir_vfs = crate::domain::VfsPath::new(&payload.connection_id, "/.trash");
    let _ = provider.create_dir(&trash_dir_vfs).await;

    let mut moved_count = 0;

    for path_str in &payload.paths {
        let vfs_path = crate::domain::VfsPath::new(&payload.connection_id, path_str);
        if let Ok(meta) = provider.stat(&vfs_path).await {
            let item_id = uuid::Uuid::new_v4().to_string();
            let trash_filename = format!("/.trash/{}_{}", &item_id[..8], meta.name);
            let dest_vfs = crate::domain::VfsPath::new(&payload.connection_id, &trash_filename);

            if provider.rename(&vfs_path, &dest_vfs).await.is_ok() {
                let is_dir = if meta.kind == crate::domain::FileKind::Directory { 1 } else { 0 };
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

                moved_count += 1;
            }
        }
    }

    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("Moved {} item(s) to Recycle Bin", moved_count),
        "count": moved_count
    })))
}

/// Restore an item from trash
pub async fn restore_trash_item(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let row: Option<(String, String, String)> = sqlx::query_as(
        "SELECT connection_id, original_path, trash_path FROM trash_items WHERE id = ?"
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

    let (connection_id, original_path, trash_path) = row.ok_or_else(|| {
        AppError::NotFound("Trash item not found".into())
    })?;

    let provider = state
        .get_provider(&connection_id)
        .await
        .ok_or_else(|| AppError::NotFound("Storage connection not found".into()))?;

    let from_vfs = crate::domain::VfsPath::new(&connection_id, &trash_path);
    let to_vfs = crate::domain::VfsPath::new(&connection_id, &original_path);

    // Move from .trash back to original
    provider
        .rename(&from_vfs, &to_vfs)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to restore file from trash: {}", e)))?;

    // Remove from trash database only after successful rename
    sqlx::query("DELETE FROM trash_items WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
        .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("Restored item to {}", original_path)
    })))
}

/// Permanently delete an item from trash
pub async fn delete_trash_item(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT connection_id, trash_path FROM trash_items WHERE id = ?"
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

    if let Some((connection_id, trash_path)) = row {
        if let Some(provider) = state.get_provider(&connection_id).await {
            let vfs_path = crate::domain::VfsPath::new(&connection_id, &trash_path);
            let _ = provider.delete(&vfs_path).await;
        }
        let _ = sqlx::query("DELETE FROM trash_items WHERE id = ?").bind(&id).execute(&state.db).await;
    }

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Item permanently deleted"
    })))
}

/// Empty entire trash
pub async fn empty_trash(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT connection_id, trash_path FROM trash_items"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    for (connection_id, trash_path) in rows {
        if let Some(provider) = state.get_provider(&connection_id).await {
            let vfs_path = crate::domain::VfsPath::new(&connection_id, &trash_path);
            let _ = provider.delete(&vfs_path).await;
        }
    }

    sqlx::query("DELETE FROM trash_items")
        .execute(&state.db)
        .await
        .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Trash emptied successfully"
    })))
}
