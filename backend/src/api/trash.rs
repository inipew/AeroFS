use crate::auth::AuthenticatedUser;
use crate::errors::AppError;
use crate::services::trash_service::{MoveToTrashRequest, TrashService};
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};

/// List all items in trash
pub async fn list_trash(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    let items = TrashService::list_trash(&state, &user).await?;
    Ok(Json(items))
}

/// Move one or more items to trash (soft delete)
pub async fn move_to_trash(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<MoveToTrashRequest>,
) -> Result<impl IntoResponse, AppError> {
    let moved_count = TrashService::move_to_trash(&state, &user, payload).await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "moved_count": moved_count,
        "message": format!("Moved {} items to trash", moved_count),
    })))
}

/// Restore an item from trash back to its original location
pub async fn restore_trash_item(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(trash_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    TrashService::restore_item(&state, &user, &trash_id).await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Item restored from trash",
    })))
}

/// Delete an item permanently from trash
pub async fn delete_trash_item(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(trash_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    if !user.is_admin {
        return Err(AppError::Forbidden(
            "Only administrators can permanently delete items from trash".into(),
        ));
    }

    let row: Option<(String, String)> =
        sqlx::query_as("SELECT connection_id, trash_path FROM trash_items WHERE id = ?")
            .bind(&trash_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

    let (connection_id, trash_path) =
        row.ok_or_else(|| AppError::NotFound("Trash item not found".into()))?;

    if let Some(provider) = state.registry.get(&connection_id).await {
        if let Ok(trash_vfs) = crate::domain::VfsPath::new(&connection_id, &trash_path) {
            let _ = provider.delete(&trash_vfs).await;
        }
    }

    sqlx::query("DELETE FROM trash_items WHERE id = ?")
        .bind(&trash_id)
        .execute(&state.db)
        .await
        .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Item permanently deleted",
    })))
}

/// Empty entire trash
pub async fn empty_trash(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    let deleted_count = TrashService::empty_trash(&state, &user).await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "deleted_count": deleted_count,
        "message": format!("Emptied {} items from trash", deleted_count),
    })))
}
