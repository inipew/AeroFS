use crate::api::extractors::{Json, Path};
use crate::auth::AuthenticatedUser;
use crate::errors::{AppError, ErrorResponse};
use crate::services::trash_service::{MoveToTrashRequest, MovedTrashItem, TrashItem, TrashService};
use crate::state::AppState;
use axum::{extract::State, response::IntoResponse};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct MoveToTrashResponse {
    pub success: bool,
    pub moved_count: usize,
    pub moved_items: Vec<MovedTrashItem>,
    pub message: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TrashActionResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct EmptyTrashResponse {
    pub success: bool,
    pub deleted_count: usize,
    pub message: String,
}

/// List all items in trash
#[utoipa::path(
    get,
    path = "/api/v1/trash",
    responses(
        (status = 200, description = "List of trash items", body = Vec<TrashItem>),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(
        ("CookieAuth" = []),
        ("BearerAuth" = [])
    ),
    tag = "trash"
)]
pub async fn list_trash(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    let items = TrashService::list_trash(&state, &user).await?;
    Ok(Json(items))
}

/// Move one or more items to trash (soft delete)
#[utoipa::path(
    post,
    path = "/api/v1/trash/move",
    request_body = MoveToTrashRequest,
    responses(
        (status = 200, description = "Items moved to trash", body = MoveToTrashResponse),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(
        ("CookieAuth" = []),
        ("BearerAuth" = [])
    ),
    tag = "trash"
)]
pub async fn move_to_trash(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<MoveToTrashRequest>,
) -> Result<impl IntoResponse, AppError> {
    let moved_items = TrashService::move_to_trash(&state, &user, payload).await?;
    let moved_count = moved_items.len();

    Ok(Json(MoveToTrashResponse {
        success: true,
        moved_count,
        moved_items,
        message: format!("Moved {} items to trash", moved_count),
    }))
}

/// Restore an item from trash back to its original location
#[utoipa::path(
    post,
    path = "/api/v1/trash/restore/{id}",
    params(
        ("id" = String, Path, description = "Trash item ID"),
    ),
    responses(
        (status = 200, description = "Item restored from trash", body = TrashActionResponse),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(
        ("CookieAuth" = []),
        ("BearerAuth" = [])
    ),
    tag = "trash"
)]
pub async fn restore_trash_item(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(trash_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    TrashService::restore_item(&state, &user, &trash_id).await?;

    Ok(Json(TrashActionResponse {
        success: true,
        message: "Item restored from trash".to_string(),
    }))
}

/// Delete an item permanently from trash
#[utoipa::path(
    delete,
    path = "/api/v1/trash/{id}",
    params(
        ("id" = String, Path, description = "Trash item ID"),
    ),
    responses(
        (status = 200, description = "Item permanently deleted", body = TrashActionResponse),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
        (status = 404, description = "Not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(
        ("CookieAuth" = []),
        ("BearerAuth" = [])
    ),
    tag = "trash"
)]
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

    Ok(Json(TrashActionResponse {
        success: true,
        message: "Item permanently deleted".to_string(),
    }))
}

/// Empty entire trash
#[utoipa::path(
    delete,
    path = "/api/v1/trash/empty",
    responses(
        (status = 200, description = "Trash emptied", body = EmptyTrashResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(
        ("CookieAuth" = []),
        ("BearerAuth" = [])
    ),
    tag = "trash"
)]
pub async fn empty_trash(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    let deleted_count = TrashService::empty_trash(&state, &user).await?;

    Ok(Json(EmptyTrashResponse {
        success: true,
        deleted_count,
        message: format!("Deleted {} items permanently", deleted_count),
    }))
}
