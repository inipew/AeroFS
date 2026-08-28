use crate::auth::permissions::{check_permission, PermissionAction};
use crate::auth::AuthenticatedUser;
use crate::domain::VfsPath;
use crate::errors::{AppError, VfsError};
use crate::filesystem::archive::{
    compress_targz, compress_zip, extract_targz, extract_zip, ArchiveFormat, ArchiveOverwriteMode,
};
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
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
    check_permission(&state.db, &user, &connection_id, PermissionAction::Create).await?;

    let provider = state.get_provider(&connection_id).await.ok_or_else(|| {
        VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
    })?;

    let dest_vfs = VfsPath::new(&connection_id, &payload.destination_file)?;
    let format = payload
        .format
        .as_deref()
        .and_then(ArchiveFormat::from_path)
        .or_else(|| ArchiveFormat::from_path(&payload.destination_file))
        .unwrap_or(ArchiveFormat::Zip);

    match format {
        ArchiveFormat::Zip => {
            compress_zip(
                &provider,
                &connection_id,
                &payload.base_path,
                &payload.relative_paths,
                &dest_vfs,
            )
            .await?;
        }
        ArchiveFormat::TarGz => {
            compress_targz(
                &provider,
                &connection_id,
                &payload.base_path,
                &payload.relative_paths,
                &dest_vfs,
            )
            .await?;
        }
    }

    crate::auth::record_audit_log(
        &state.db,
        Some(&user.id),
        "ARCHIVE_COMPRESS",
        Some(&connection_id),
        Some(&dest_vfs.path),
        "SUCCESS",
        None,
        Some(&format!("Created archive: {}", dest_vfs.path)),
    )
    .await;

    Ok((
        StatusCode::CREATED,
        Json(ArchiveResponse {
            success: true,
            message: format!("Archive created: {}", dest_vfs.path),
            entries_count: Some(payload.relative_paths.len()),
            skipped_count: None,
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
    // P1 #15: Double permission check (Create dirs + Write/Overwrite files)
    check_permission(&state.db, &user, &connection_id, PermissionAction::Create).await?;
    check_permission(&state.db, &user, &connection_id, PermissionAction::Write).await?;

    let provider = state.get_provider(&connection_id).await.ok_or_else(|| {
        VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
    })?;

    let archive_vfs = VfsPath::new(&connection_id, &payload.archive_path)?;
    let format = payload
        .format
        .as_deref()
        .and_then(ArchiveFormat::from_path)
        .or_else(|| ArchiveFormat::from_path(&payload.archive_path))
        .unwrap_or(ArchiveFormat::Zip);

    let overwrite_mode = payload.overwrite_mode.unwrap_or_default();
    let (count, skipped) = match format {
        ArchiveFormat::Zip => {
            extract_zip(
                &provider,
                &archive_vfs,
                &payload.destination_dir,
                overwrite_mode,
            )
            .await?
        }
        ArchiveFormat::TarGz => {
            extract_targz(
                &provider,
                &archive_vfs,
                &payload.destination_dir,
                overwrite_mode,
            )
            .await?
        }
    };

    crate::auth::record_audit_log(
        &state.db,
        Some(&user.id),
        "ARCHIVE_EXTRACT",
        Some(&connection_id),
        Some(&archive_vfs.path),
        "SUCCESS",
        None,
        Some(&format!(
            "Extracted {} items (skipped {}) to {}",
            count, skipped, payload.destination_dir
        )),
    )
    .await;

    Ok((
        StatusCode::OK,
        Json(ArchiveResponse {
            success: true,
            message: format!("Extracted {} item(s) to {}", count, payload.destination_dir),
            entries_count: Some(count),
            skipped_count: Some(skipped),
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
    axum::extract::Query(query): axum::extract::Query<ListArchiveQuery>,
) -> Result<impl IntoResponse, AppError> {
    check_permission(&state.db, &user, &connection_id, PermissionAction::Read).await?;

    let provider = state.get_provider(&connection_id).await.ok_or_else(|| {
        VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
    })?;

    let archive_vfs = VfsPath::new(&connection_id, &query.archive_path)?;
    let subpath = query.subpath.unwrap_or_default();

    let entries =
        crate::filesystem::archive::list_virtual_archive_entries(&provider, &archive_vfs, &subpath)
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
    axum::extract::Query(query): axum::extract::Query<ReadArchiveQuery>,
) -> Result<impl IntoResponse, AppError> {
    check_permission(&state.db, &user, &connection_id, PermissionAction::Read).await?;

    let provider = state.get_provider(&connection_id).await.ok_or_else(|| {
        VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
    })?;

    let archive_vfs = VfsPath::new(&connection_id, &query.archive_path)?;
    let (file_name, bytes) = crate::filesystem::archive::read_virtual_archive_entry(
        &provider,
        &archive_vfs,
        &query.entry_path,
    )
    .await?;

    let mime_type = mime_guess::from_path(&file_name)
        .first_or_octet_stream()
        .to_string();

    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        mime_type
            .parse()
            .unwrap_or_else(|_| axum::http::HeaderValue::from_static("application/octet-stream")),
    );
    headers.insert(
        axum::http::header::CONTENT_DISPOSITION,
        format!("inline; filename=\"{}\"", file_name)
            .parse()
            .unwrap_or_else(|_| axum::http::HeaderValue::from_static("inline")),
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
    // P1 #15: Double permission check (Create dirs + Write/Overwrite files)
    check_permission(&state.db, &user, &connection_id, PermissionAction::Create).await?;
    check_permission(&state.db, &user, &connection_id, PermissionAction::Write).await?;

    let provider = state.get_provider(&connection_id).await.ok_or_else(|| {
        VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
    })?;

    let archive_vfs = VfsPath::new(&connection_id, &payload.archive_path)?;
    let overwrite_mode = payload.overwrite_mode.unwrap_or_default();
    let (count, skipped) = crate::filesystem::archive::extract_selected_archive_entries(
        &provider,
        &archive_vfs,
        &payload.destination_dir,
        &payload.entries,
        overwrite_mode,
    )
    .await?;

    crate::auth::record_audit_log(
        &state.db,
        Some(&user.id),
        "ARCHIVE_EXTRACT_SELECTED",
        Some(&connection_id),
        Some(&archive_vfs.path),
        "SUCCESS",
        None,
        Some(&format!(
            "Extracted {} selected item(s) (skipped {}) to {}",
            count, skipped, payload.destination_dir
        )),
    )
    .await;

    Ok((
        StatusCode::OK,
        Json(ArchiveResponse {
            success: true,
            message: format!(
                "Extracted {} selected item(s) to {}",
                count, payload.destination_dir
            ),
            entries_count: Some(count),
            skipped_count: Some(skipped),
        }),
    ))
}
