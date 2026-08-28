use crate::auth::AuthenticatedUser;
use crate::errors::AppError;
use crate::filesystem::archive::ArchiveOverwriteMode;
use crate::services::ArchiveService;
use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

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

/// Compress files into a ZIP or TAR.GZ archive
pub async fn compress_files(
    State(state): State<AppState>,
    Path(connection_id): Path<String>,
    user: AuthenticatedUser,
    Json(payload): Json<CompressRequest>,
) -> Result<impl IntoResponse, AppError> {
    let res = ArchiveService::compress(
        &state,
        &user,
        &connection_id,
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
pub async fn extract_archive_endpoint(
    State(state): State<AppState>,
    Path(connection_id): Path<String>,
    user: AuthenticatedUser,
    Json(payload): Json<ExtractRequest>,
) -> Result<impl IntoResponse, AppError> {
    let overwrite_mode = payload.overwrite_mode.unwrap_or_default();
    let res = ArchiveService::extract(
        &state,
        &user,
        &connection_id,
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

#[derive(Debug, Deserialize, ToSchema)]
pub struct ListArchiveQuery {
    pub archive_path: String,
    pub subpath: Option<String>,
}

/// List virtual directory contents inside an archive without full extraction
pub async fn list_virtual_archive_endpoint(
    State(state): State<AppState>,
    Path(connection_id): Path<String>,
    user: AuthenticatedUser,
    Query(query): Query<ListArchiveQuery>,
) -> Result<impl IntoResponse, AppError> {
    let subpath = query.subpath.unwrap_or_default();
    let entries =
        ArchiveService::list_virtual(&state, &user, &connection_id, &query.archive_path, &subpath)
            .await?;

    Ok((StatusCode::OK, Json(entries)))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReadArchiveQuery {
    pub archive_path: String,
    pub entry_path: String,
}

/// Stream or download a single entry directly from an archive
pub async fn read_virtual_archive_entry_endpoint(
    State(state): State<AppState>,
    Path(connection_id): Path<String>,
    user: AuthenticatedUser,
    Query(query): Query<ReadArchiveQuery>,
) -> Result<impl IntoResponse, AppError> {
    let (file_name, bytes) = ArchiveService::read_virtual_entry(
        &state,
        &user,
        &connection_id,
        &query.archive_path,
        &query.entry_path,
    )
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
pub async fn extract_selected_archive_endpoint(
    State(state): State<AppState>,
    Path(connection_id): Path<String>,
    user: AuthenticatedUser,
    Json(payload): Json<ExtractSelectedRequest>,
) -> Result<impl IntoResponse, AppError> {
    let overwrite_mode = payload.overwrite_mode.unwrap_or_default();
    let res = ArchiveService::extract_selected(
        &state,
        &user,
        &connection_id,
        &payload.archive_path,
        &payload.destination_dir,
        &payload.entries,
        None,
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
