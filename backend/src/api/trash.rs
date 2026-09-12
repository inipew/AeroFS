use crate::api::extractors::{Json, Path};
use crate::auth::AuthenticatedUser;
use crate::errors::{AppError, ErrorResponse};
use crate::services::trash_service::{MoveToTrashRequest, MovedTrashItem, TrashItem};
use crate::state::TrashState;
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

#[utoipa::path(
    get,
    path = "/api/v1/trash",
    responses(
        (status = 200, description = "List of trash items", body = Vec<TrashItem>),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(("CookieAuth" = []), ("BearerAuth" = [])),
    tag = "trash"
)]
pub async fn list_trash(
    State(state): State<TrashState>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    Ok(Json(state.service.list_trash(&user).await?))
}

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
    security(("CookieAuth" = []), ("BearerAuth" = [])),
    tag = "trash"
)]
pub async fn move_to_trash(
    State(state): State<TrashState>,
    user: AuthenticatedUser,
    Json(payload): Json<MoveToTrashRequest>,
) -> Result<impl IntoResponse, AppError> {
    let moved_items = state.service.move_to_trash(&user, payload).await?;
    let moved_count = moved_items.len();
    Ok(Json(MoveToTrashResponse {
        success: true,
        moved_count,
        moved_items,
        message: format!("Moved {} items to trash", moved_count),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/trash/restore/{id}",
    params(("id" = String, Path, description = "Trash item ID")),
    responses(
        (status = 200, description = "Item restored from trash", body = TrashActionResponse),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(("CookieAuth" = []), ("BearerAuth" = [])),
    tag = "trash"
)]
pub async fn restore_trash_item(
    State(state): State<TrashState>,
    user: AuthenticatedUser,
    Path(trash_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    state.service.restore_item(&user, &trash_id).await?;
    Ok(Json(TrashActionResponse {
        success: true,
        message: "Item restored from trash".to_string(),
    }))
}

#[utoipa::path(
    delete,
    path = "/api/v1/trash/{id}",
    params(("id" = String, Path, description = "Trash item ID")),
    responses(
        (status = 200, description = "Item permanently deleted", body = TrashActionResponse),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
        (status = 404, description = "Not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(("CookieAuth" = []), ("BearerAuth" = [])),
    tag = "trash"
)]
pub async fn delete_trash_item(
    State(state): State<TrashState>,
    user: AuthenticatedUser,
    Path(trash_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    state.service.delete_permanently(&user, &trash_id).await?;
    Ok(Json(TrashActionResponse {
        success: true,
        message: "Item permanently deleted".to_string(),
    }))
}

#[utoipa::path(
    delete,
    path = "/api/v1/trash/empty",
    responses(
        (status = 200, description = "Trash emptied", body = EmptyTrashResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(("CookieAuth" = []), ("BearerAuth" = [])),
    tag = "trash"
)]
pub async fn empty_trash(
    State(state): State<TrashState>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    let deleted_count = state.service.empty_trash(&user).await?;
    Ok(Json(EmptyTrashResponse {
        success: true,
        deleted_count,
        message: format!("Deleted {} items permanently", deleted_count),
    }))
}
