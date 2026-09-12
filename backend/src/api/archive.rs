use crate::api::extractors::{Json, Path, Query};
use crate::auth::AuthenticatedUser;
use crate::domain::{Actor, ConnectionId};
use crate::errors::{AppError, ErrorResponse};
use crate::filesystem::archive::{ArchiveOverwriteMode, VirtualArchiveEntry};
use crate::state::ArchiveState;
use axum::{
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Deserialize, ToSchema)]
pub struct CompressRequest {
    pub base_path: String,
    pub relative_paths: Vec<String>,
    pub destination_file: String,
    pub format: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ExtractRequest {
    pub archive_path: String,
    pub destination_dir: String,
    pub format: Option<String>,
    pub overwrite_mode: Option<ArchiveOverwriteMode>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ArchiveResponse {
    pub success: bool,
    pub message: String,
    pub entries_count: Option<usize>,
    pub skipped_count: Option<usize>,
}

fn actor_from_user(user: &AuthenticatedUser) -> Actor {
    Actor {
        id: user.id().to_string(),
        username: user.username().to_string(),
        is_admin: user.is_admin(),
    }
}

fn connection_id(raw: String) -> Result<ConnectionId, AppError> {
    ConnectionId::new(raw).map_err(|error| AppError::BadRequest(error.to_string()))
}

/// Compress files into a ZIP or TAR.GZ archive
#[utoipa::path(
    post,
    path = "/api/v1/connections/{connection_id}/archive/compress",
    params(
        ("connection_id" = String, Path, description = "Connection ID"),
    ),
    request_body = CompressRequest,
    responses(
        (status = 201, description = "Archive created", body = ArchiveResponse),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(
        ("CookieAuth" = []),
        ("BearerAuth" = [])
    ),
    tag = "archive"
)]
pub async fn compress_files(
    State(state): State<ArchiveState>,
    Path(connection_id_raw): Path<String>,
    user: AuthenticatedUser,
    Json(payload): Json<CompressRequest>,
) -> Result<impl IntoResponse, AppError> {
    let actor = actor_from_user(&user);
    let connection = connection_id(connection_id_raw)?;
    let res = state
        .service
        .compress(
            &actor,
            &connection,
            &payload.base_path,
            &payload.relative_paths,
            &payload.destination_file,
            payload.format.as_deref(),
        )
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(ArchiveResponse {
            success: res.success,
            message: res.message,
            entries_count: res.entries_count,
            skipped_count: res.skipped_count,
        }),
    ))
}

/// Extract an archive into a target directory (Requires Create + Write permissions)
#[utoipa::path(
    post,
    path = "/api/v1/connections/{connection_id}/archive/extract",
    params(
        ("connection_id" = String, Path, description = "Connection ID"),
    ),
    request_body = ExtractRequest,
    responses(
        (status = 200, description = "Archive extracted", body = ArchiveResponse),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(
        ("CookieAuth" = []),
        ("BearerAuth" = [])
    ),
    tag = "archive"
)]
pub async fn extract_archive_endpoint(
    State(state): State<ArchiveState>,
    Path(connection_id_raw): Path<String>,
    user: AuthenticatedUser,
    Json(payload): Json<ExtractRequest>,
) -> Result<impl IntoResponse, AppError> {
    let actor = actor_from_user(&user);
    let connection = connection_id(connection_id_raw)?;
    let overwrite_mode = payload.overwrite_mode.unwrap_or_default();
    let res = state
        .service
        .extract(
            &actor,
            &connection,
            &payload.archive_path,
            &payload.destination_dir,
            payload.format.as_deref(),
            overwrite_mode,
        )
        .await?;

    Ok((
        StatusCode::OK,
        Json(ArchiveResponse {
            success: res.success,
            message: res.message,
            entries_count: res.entries_count,
            skipped_count: res.skipped_count,
        }),
    ))
}

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct ListArchiveQuery {
    pub archive_path: String,
    pub subpath: Option<String>,
}

/// List virtual directory contents inside an archive without full extraction
#[utoipa::path(
    get,
    path = "/api/v1/connections/{connection_id}/archive/entries",
    params(
        ("connection_id" = String, Path, description = "Connection ID"),
        ListArchiveQuery
    ),
    responses(
        (status = 200, description = "Virtual archive entries", body = Vec<VirtualArchiveEntry>),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(
        ("CookieAuth" = []),
        ("BearerAuth" = [])
    ),
    tag = "archive"
)]
pub async fn list_virtual_archive_endpoint(
    State(state): State<ArchiveState>,
    Path(connection_id_raw): Path<String>,
    user: AuthenticatedUser,
    Query(query): Query<ListArchiveQuery>,
) -> Result<impl IntoResponse, AppError> {
    let actor = actor_from_user(&user);
    let connection = connection_id(connection_id_raw)?;
    let subpath = query.subpath.unwrap_or_default();
    let entries = state
        .service
        .list_virtual(&actor, &connection, &query.archive_path, &subpath)
        .await?;

    Ok((StatusCode::OK, Json(entries)))
}

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct ReadArchiveQuery {
    pub archive_path: String,
    pub entry_path: String,
}

/// Stream or download a single entry directly from an archive
#[utoipa::path(
    get,
    path = "/api/v1/connections/{connection_id}/archive/read",
    params(
        ("connection_id" = String, Path, description = "Connection ID"),
        ReadArchiveQuery
    ),
    responses(
        (status = 200, description = "Virtual archive entry file content", content_type = "application/octet-stream"),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(
        ("CookieAuth" = []),
        ("BearerAuth" = [])
    ),
    tag = "archive"
)]
pub async fn read_virtual_archive_entry_endpoint(
    State(state): State<ArchiveState>,
    Path(connection_id_raw): Path<String>,
    user: AuthenticatedUser,
    Query(query): Query<ReadArchiveQuery>,
) -> Result<impl IntoResponse, AppError> {
    let actor = actor_from_user(&user);
    let connection = connection_id(connection_id_raw)?;
    let (file_name, bytes) = state
        .service
        .read_virtual_entry(&actor, &connection, &query.archive_path, &query.entry_path)
        .await?;

    let mime_type = mime_guess::from_path(&file_name)
        .first_or_octet_stream()
        .to_string();

    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        mime_type
            .parse()
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    headers.insert(
        axum::http::header::CONTENT_DISPOSITION,
        format!("inline; filename=\"{}\"", file_name)
            .parse()
            .unwrap_or_else(|_| HeaderValue::from_static("inline")),
    );

    Ok((StatusCode::OK, headers, bytes))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ExtractSelectedRequest {
    pub archive_path: String,
    pub destination_dir: String,
    pub entries: Vec<String>,
    pub overwrite_mode: Option<ArchiveOverwriteMode>,
}

/// Extract specific selected entries from an archive into a destination directory (Requires Create + Write permissions)
#[utoipa::path(
    post,
    path = "/api/v1/connections/{connection_id}/archive/extract-selected",
    params(
        ("connection_id" = String, Path, description = "Connection ID"),
    ),
    request_body = ExtractSelectedRequest,
    responses(
        (status = 200, description = "Selected entries extracted", body = ArchiveResponse),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(
        ("CookieAuth" = []),
        ("BearerAuth" = [])
    ),
    tag = "archive"
)]
pub async fn extract_selected_archive_endpoint(
    State(state): State<ArchiveState>,
    Path(connection_id_raw): Path<String>,
    user: AuthenticatedUser,
    Json(payload): Json<ExtractSelectedRequest>,
) -> Result<impl IntoResponse, AppError> {
    let actor = actor_from_user(&user);
    let connection = connection_id(connection_id_raw)?;
    let overwrite_mode = payload.overwrite_mode.unwrap_or_default();
    let res = state
        .service
        .extract_selected(
            &actor,
            &connection,
            &payload.archive_path,
            &payload.destination_dir,
            &payload.entries,
            overwrite_mode,
        )
        .await?;

    Ok((
        StatusCode::OK,
        Json(ArchiveResponse {
            success: res.success,
            message: res.message,
            entries_count: res.entries_count,
            skipped_count: res.skipped_count,
        }),
    ))
}
