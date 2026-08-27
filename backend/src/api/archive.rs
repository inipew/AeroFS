use crate::auth::AuthenticatedUser;
use crate::domain::VfsPath;
use crate::errors::{AppError, VfsError};
use crate::filesystem::archive::{compress_targz, compress_zip, extract_targz, extract_zip, ArchiveFormat};
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
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ArchiveResponse {
    pub success: bool,
    pub message: String,
    pub entries_count: Option<usize>,
}

/// Compress files into a ZIP or TAR.GZ archive
pub async fn compress_files(
    State(state): State<AppState>,
    Path(connection_id): Path<String>,
    user: AuthenticatedUser,
    Json(payload): Json<CompressRequest>,
) -> Result<impl IntoResponse, AppError> {
    let provider = state
        .get_provider(&connection_id)
        .await
        .ok_or_else(|| VfsError::ConnectionError(format!("Connection '{}' not found", connection_id)))?;

    let dest_vfs = VfsPath::new(&connection_id, &payload.destination_file);
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
        }),
    ))
}

/// Extract an archive into a target directory
pub async fn extract_archive_endpoint(
    State(state): State<AppState>,
    Path(connection_id): Path<String>,
    user: AuthenticatedUser,
    Json(payload): Json<ExtractRequest>,
) -> Result<impl IntoResponse, AppError> {
    let provider = state
        .get_provider(&connection_id)
        .await
        .ok_or_else(|| VfsError::ConnectionError(format!("Connection '{}' not found", connection_id)))?;

    let archive_vfs = VfsPath::new(&connection_id, &payload.archive_path);
    let format = payload
        .format
        .as_deref()
        .and_then(ArchiveFormat::from_path)
        .or_else(|| ArchiveFormat::from_path(&payload.archive_path))
        .unwrap_or(ArchiveFormat::Zip);

    let count = match format {
        ArchiveFormat::Zip => {
            extract_zip(&provider, &archive_vfs, &payload.destination_dir).await?
        }
        ArchiveFormat::TarGz => {
            extract_targz(&provider, &archive_vfs, &payload.destination_dir).await?
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
        Some(&format!("Extracted {} items to {}", count, payload.destination_dir)),
    )
    .await;

    Ok((
        StatusCode::OK,
        Json(ArchiveResponse {
            success: true,
            message: format!("Extracted {} item(s) to {}", count, payload.destination_dir),
            entries_count: Some(count),
        }),
    ))
}
