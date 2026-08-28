use crate::auth::{check_permission, AuthenticatedUser, PermissionAction};
use crate::domain::{DirectoryListing, FileKind, VfsPath};
use crate::errors::{AppError, VfsError};
use crate::state::AppState;
use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{
        header::{self, CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE, ETAG, LAST_MODIFIED},
        HeaderMap, StatusCode,
    },
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use tokio_util::io::ReaderStream;
use utoipa::{IntoParams, ToSchema};

#[cfg(unix)]
pub fn get_available_disk_space(path: &std::path::Path) -> Option<u64> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let c_path = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) } == 0 {
        Some((stat.f_bavail as u64) * (stat.f_frsize as u64))
    } else {
        None
    }
}

#[cfg(not(unix))]
pub fn get_available_disk_space(_path: &std::path::Path) -> Option<u64> {
    None
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListFilesQuery {
    pub path: Option<String>,
    pub show_hidden: Option<bool>,
    pub sort: Option<String>,
    pub order: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct PathQuery {
    pub path: String,
    pub download: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateEntryRequest {
    pub path: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateContentRequest {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DeleteRequest {
    pub paths: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ChmodRequest {
    pub path: String,
    pub mode: u32,
    pub recursive: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct TransferRequest {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SuccessResponse {
    pub success: bool,
    pub message: String,
}

/// List files and directories in a given path for a connection
pub async fn list_files(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(connection_id): Path<String>,
    Query(query): Query<ListFilesQuery>,
) -> Result<impl IntoResponse, AppError> {
    check_permission(&state.db, &user, &connection_id, PermissionAction::Read).await?;

    let provider = state.get_provider(&connection_id).await.ok_or_else(|| {
        VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
    })?;

    let raw_path = query.path.unwrap_or_else(|| "/".to_string());
    let vfs_path = VfsPath::new(&connection_id, raw_path)?;

    let mut entries = provider.list(&vfs_path).await?;

    // Filter hidden if not requested
    let show_hidden = match query.show_hidden {
        Some(val) => val,
        None => {
            if let Some(sys_val) = state.get_system_setting("show_hidden_default").await {
                sys_val == "true"
            } else {
                state.config.filesystem.show_hidden_default
            }
        }
    };
    if !show_hidden {
        entries.retain(|e| !e.is_hidden);
    }

    // Sort entries
    let sort_field = query.sort.as_deref().unwrap_or("name");
    let is_desc = query.order.as_deref() == Some("desc");

    entries.sort_by(|a, b| {
        let cmp = match (a.kind, b.kind) {
            (FileKind::Directory, FileKind::File) => std::cmp::Ordering::Less,
            (FileKind::File, FileKind::Directory) => std::cmp::Ordering::Greater,
            _ => match sort_field {
                "size" => a.size.unwrap_or(0).cmp(&b.size.unwrap_or(0)),
                "date" => a.modified_at.cmp(&b.modified_at),
                "type" => {
                    let ext_a = a.name.split('.').next_back().unwrap_or("");
                    let ext_b = b.name.split('.').next_back().unwrap_or("");
                    ext_a.to_lowercase().cmp(&ext_b.to_lowercase())
                }
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            },
        };
        if is_desc {
            cmp.reverse()
        } else {
            cmp
        }
    });

    let max_entries = state.config.limits.max_directory_entries;
    let total = entries.len();
    if entries.len() > max_entries {
        entries.truncate(max_entries);
    }

    Ok(Json(DirectoryListing {
        path: vfs_path.path,
        connection_id,
        entries,
        total_count: total,
        next_cursor: None,
    }))
}

/// Get detailed metadata for a file or directory
pub async fn stat_file(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(connection_id): Path<String>,
    Query(query): Query<PathQuery>,
) -> Result<impl IntoResponse, AppError> {
    check_permission(&state.db, &user, &connection_id, PermissionAction::Read).await?;

    let provider = state.get_provider(&connection_id).await.ok_or_else(|| {
        VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
    })?;

    let vfs_path = VfsPath::new(&connection_id, query.path)?;
    let meta = provider.stat(&vfs_path).await?;

    Ok(Json(meta))
}

pub use stat_file as get_metadata;

/// Stream file content or download with HTTP Range support (206 Partial Content)
pub async fn get_file_content(
    State(state): State<AppState>,
    req_headers: HeaderMap,
    user: AuthenticatedUser,
    Path(connection_id): Path<String>,
    Query(query): Query<PathQuery>,
) -> Result<impl IntoResponse, AppError> {
    let action = if query.download.unwrap_or(false) {
        PermissionAction::Download
    } else {
        PermissionAction::Read
    };
    check_permission(&state.db, &user, &connection_id, action).await?;

    let provider = state.get_provider(&connection_id).await.ok_or_else(|| {
        VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
    })?;

    let vfs_path = VfsPath::new(&connection_id, query.path)?;
    let meta = provider.stat(&vfs_path).await?;

    if meta.kind != FileKind::File {
        return Err(AppError::BadRequest("Target is not a regular file".into()));
    }

    let file_size = meta.size;
    let mime = meta
        .mime_type
        .unwrap_or_else(|| "application/octet-stream".to_string());

    let mut resp_headers = HeaderMap::new();
    resp_headers.insert(header::ACCEPT_RANGES, "bytes".parse().unwrap());
    resp_headers.insert(CONTENT_TYPE, mime.parse().unwrap());
    resp_headers.insert(ETAG, meta.etag.parse().unwrap());
    resp_headers.insert(
        header::CACHE_CONTROL,
        "private, max-age=3600, must-revalidate".parse().unwrap(),
    );

    if let Some(mtime) = meta.modified_at {
        resp_headers.insert(LAST_MODIFIED, mtime.to_rfc2822().parse().unwrap());
    }

    // Security Sandbox: Isolate inline HTML/SVG/JS preview from host origin (XSS mitigation)
    let is_active_content = mime == "text/html"
        || mime == "image/svg+xml"
        || mime == "application/xml"
        || mime == "text/xml"
        || mime == "text/javascript"
        || mime == "application/javascript";

    if is_active_content && !query.download.unwrap_or(false) {
        resp_headers.insert(
            header::HeaderName::from_static("content-security-policy"),
            "default-src 'none'; sandbox".parse().unwrap(),
        );
        resp_headers.insert(
            header::HeaderName::from_static("x-content-type-options"),
            "nosniff".parse().unwrap(),
        );
    }

    if query.download.unwrap_or(false) {
        let ascii_fallback = meta
            .name
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '_' || *c == '-')
            .collect::<String>();
        let fallback = if ascii_fallback.is_empty() {
            "download".to_string()
        } else {
            ascii_fallback
        };
        let encoded_utf8 = urlencoding::encode(&meta.name);
        let disposition = format!(
            "attachment; filename=\"{}\"; filename*=UTF-8''{}",
            fallback, encoded_utf8
        );
        resp_headers.insert(CONTENT_DISPOSITION, disposition.parse().unwrap());

        crate::auth::record_audit_log(
            &state.db,
            Some(&user.id),
            "FILE_DOWNLOAD",
            Some(&connection_id),
            Some(&vfs_path.path),
            "SUCCESS",
            None,
            Some(&format!("Downloaded: {}", vfs_path.path)),
        )
        .await;
    }

    // Handle ETag conditional caching: 304 Not Modified
    if let Some(if_none_match) = req_headers
        .get(header::IF_NONE_MATCH)
        .and_then(|h| h.to_str().ok())
    {
        let clean_client = if_none_match.trim().trim_matches('"');
        let clean_server = meta.etag.trim().trim_matches('"');
        if clean_client == clean_server || if_none_match == "*" {
            return Ok((StatusCode::NOT_MODIFIED, resp_headers, Body::empty()));
        }
    }

    // Handle HTTP Range header for seeking in video/audio players (RFC 7233 compliant)
    if let Some(range_val) = req_headers.get(header::RANGE).and_then(|r| r.to_str().ok()) {
        if let Some(range_spec) = range_val.strip_prefix("bytes=") {
            let parts: Vec<&str> = range_spec.split('-').collect();
            if parts.len() == 2 {
                let (start, end) = if parts[0].is_empty() {
                    // Suffix range: bytes=-500 (last 500 bytes)
                    let suffix_len: u64 = parts[1].parse().unwrap_or(0);
                    if suffix_len == 0 {
                        (0, 0)
                    } else {
                        let s = file_size.saturating_sub(suffix_len);
                        (s, file_size.saturating_sub(1))
                    }
                } else if parts[1].is_empty() {
                    // Open-ended range: bytes=500- (from 500 to EOF)
                    let s: u64 = parts[0].parse().unwrap_or(0);
                    (s, file_size.saturating_sub(1))
                } else {
                    // Bounded range: bytes=100-200
                    let s: u64 = parts[0].parse().unwrap_or(0);
                    let e: u64 = parts[1]
                        .parse()
                        .unwrap_or(file_size.saturating_sub(1))
                        .min(file_size.saturating_sub(1));
                    (s, e)
                };

                if start <= end && start < file_size {
                    let chunk_len = end - start + 1;

                    // True O(1) range read via provider.read_range
                    let stream = provider.read_range(&vfs_path, start, chunk_len).await?;
                    let body = Body::from_stream(ReaderStream::new(stream));

                    resp_headers.insert(CONTENT_LENGTH, chunk_len.to_string().parse().unwrap());
                    resp_headers.insert(
                        header::CONTENT_RANGE,
                        format!("bytes {}-{}/{}", start, end, file_size)
                            .parse()
                            .unwrap(),
                    );
                    return Ok((StatusCode::PARTIAL_CONTENT, resp_headers, body));
                } else {
                    // 416 Range Not Satisfiable
                    let mut unsat_headers = HeaderMap::new();
                    unsat_headers.insert(header::ACCEPT_RANGES, "bytes".parse().unwrap());
                    unsat_headers.insert(
                        header::CONTENT_RANGE,
                        format!("bytes */{}", file_size).parse().unwrap(),
                    );
                    return Ok((
                        StatusCode::RANGE_NOT_SATISFIABLE,
                        unsat_headers,
                        Body::empty(),
                    ));
                }
            }
        }
    }

    // Default full stream (200 OK)
    let stream = provider.read_stream(&vfs_path).await?;
    let body = Body::from_stream(ReaderStream::new(stream));
    resp_headers.insert(CONTENT_LENGTH, file_size.to_string().parse().unwrap());

    Ok((StatusCode::OK, resp_headers, body))
}

/// Update file content with optimistic concurrency control (If-Match header)
pub async fn update_file_content(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthenticatedUser,
    Path(connection_id): Path<String>,
    Json(payload): Json<UpdateContentRequest>,
) -> Result<impl IntoResponse, AppError> {
    check_permission(&state.db, &user, &connection_id, PermissionAction::Write).await?;

    let provider = state.get_provider(&connection_id).await.ok_or_else(|| {
        VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
    })?;

    let vfs_path = VfsPath::new(&connection_id, &payload.path)?;

    // Limit check: enforce max_editable_size (dynamic check)
    let max_editable_bytes =
        if let Some(custom) = state.get_system_setting("max_editable_size").await {
            custom
                .parse()
                .unwrap_or(state.config.limits.max_editable_size)
        } else {
            state.config.limits.max_editable_size
        };
    if payload.content.len() as u64 > max_editable_bytes {
        return Err(AppError::PayloadTooLarge(format!(
            "File content length ({} bytes) exceeds maximum editable size of {} bytes",
            payload.content.len(),
            max_editable_bytes
        )));
    }

    // Optimistic Concurrency check
    if let Ok(meta) = provider.stat(&vfs_path).await {
        if let Some(if_match) = headers.get(header::IF_MATCH) {
            if let Ok(expected_etag) = if_match.to_str() {
                let clean_expected = expected_etag.trim().trim_matches('"');
                let clean_actual = meta.etag.trim().trim_matches('"');
                if clean_expected != clean_actual && expected_etag != "*" {
                    return Err(AppError::Conflict(format!(
                        "File was modified externally. Expected ETag: {}, Current ETag: {}",
                        clean_expected, clean_actual
                    )));
                }
            }
        }
    }

    // Destination permission preservation: capture existing permissions or inherit from parent
    let target_perms = crate::domain::resolve_destination_permissions(
        &provider,
        &vfs_path,
        false,
        crate::domain::PermissionInheritanceMode::InheritExistingOrParent,
    )
    .await;

    // Atomic write safety: write to temporary path first then rename to prevent corrupt target on disconnect
    let bytes = payload.content.into_bytes();
    let tmp_vfs = VfsPath::new(&connection_id, format!("{}.aerofs.tmp", vfs_path.path))?;
    let cursor = Cursor::new(bytes.clone());

    if provider
        .write_stream(&tmp_vfs, Box::new(cursor))
        .await
        .is_ok()
    {
        if let Some(ref perms) = target_perms {
            let _ = provider.set_permissions(&tmp_vfs, perms).await;
        }
        if provider.rename(&tmp_vfs, &vfs_path).await.is_err() {
            let fallback = Cursor::new(bytes);
            provider.write_stream(&vfs_path, Box::new(fallback)).await?;
            if let Some(ref perms) = target_perms {
                let _ = provider.set_permissions(&vfs_path, perms).await;
            }
            let _ = provider.delete(&tmp_vfs).await;
        } else if let Some(ref perms) = target_perms {
            let _ = provider.set_permissions(&vfs_path, perms).await;
        }
    } else {
        let fallback = Cursor::new(bytes);
        provider.write_stream(&vfs_path, Box::new(fallback)).await?;
        if let Some(ref perms) = target_perms {
            let _ = provider.set_permissions(&vfs_path, perms).await;
        }
    }

    crate::auth::record_audit_log(
        &state.db,
        Some(&user.id),
        "FILE_UPDATE",
        Some(&connection_id),
        Some(&vfs_path.path),
        "SUCCESS",
        None,
        Some(&format!("Updated file content: {}", vfs_path.path)),
    )
    .await;

    // Get fresh ETag after write
    let new_meta = provider.stat(&vfs_path).await?;
    let mut resp_headers = HeaderMap::new();
    resp_headers.insert("access-control-expose-headers", "ETag".parse().unwrap());
    if let Ok(val) = new_meta.etag.parse() {
        resp_headers.insert(header::ETAG, val);
    }

    Ok((
        StatusCode::OK,
        resp_headers,
        Json(SuccessResponse {
            success: true,
            message: format!("File updated: {}", vfs_path.path),
        }),
    ))
}

/// Create an empty file
pub async fn create_file(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(connection_id): Path<String>,
    Json(payload): Json<CreateEntryRequest>,
) -> Result<impl IntoResponse, AppError> {
    check_permission(&state.db, &user, &connection_id, PermissionAction::Create).await?;

    let provider = state.get_provider(&connection_id).await.ok_or_else(|| {
        VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
    })?;

    let vfs_path = VfsPath::new(&connection_id, payload.path)?;
    provider.create_file(&vfs_path).await?;

    if let Some(perms) = crate::domain::resolve_destination_permissions(
        &provider,
        &vfs_path,
        false,
        crate::domain::PermissionInheritanceMode::InheritParent,
    )
    .await
    {
        let _ = provider.set_permissions(&vfs_path, &perms).await;
    }

    Ok((
        StatusCode::CREATED,
        Json(SuccessResponse {
            success: true,
            message: format!("File created: {}", vfs_path.path),
        }),
    ))
}

/// Create a directory
pub async fn create_directory(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(connection_id): Path<String>,
    Json(payload): Json<CreateEntryRequest>,
) -> Result<impl IntoResponse, AppError> {
    check_permission(&state.db, &user, &connection_id, PermissionAction::Create).await?;

    let provider = state.get_provider(&connection_id).await.ok_or_else(|| {
        VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
    })?;

    let vfs_path = VfsPath::new(&connection_id, payload.path)?;
    provider.create_dir(&vfs_path).await?;

    if let Some(perms) = crate::domain::resolve_destination_permissions(
        &provider,
        &vfs_path,
        true,
        crate::domain::PermissionInheritanceMode::InheritParent,
    )
    .await
    {
        let _ = provider.set_permissions(&vfs_path, &perms).await;
    }

    Ok((
        StatusCode::CREATED,
        Json(SuccessResponse {
            success: true,
            message: format!("Directory created: {}", vfs_path.path),
        }),
    ))
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct DeleteResultItem {
    pub path: String,
    pub error: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct DeleteResponse {
    pub success: bool,
    pub succeeded: Vec<String>,
    pub failed: Vec<DeleteResultItem>,
    pub message: String,
}

/// Delete one or more files / directories with bounded concurrency (8 workers)
pub async fn delete_files(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(connection_id): Path<String>,
    Json(payload): Json<DeleteRequest>,
) -> Result<impl IntoResponse, AppError> {
    check_permission(&state.db, &user, &connection_id, PermissionAction::Delete).await?;

    let provider = state.get_provider(&connection_id).await.ok_or_else(|| {
        VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
    })?;

    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(8));
    let mut tasks = tokio::task::JoinSet::new();

    for p in payload.paths {
        let provider = provider.clone();
        let conn_id = connection_id.clone();
        let sem = semaphore.clone();
        tasks.spawn(async move {
            let _permit = sem.acquire().await;
            let vfs_path = match VfsPath::new(&conn_id, &p) {
                Ok(v) => v,
                Err(e) => return (p, Err(e)),
            };
            let res = provider.delete(&vfs_path).await;
            (p, res)
        });
    }

    let mut succeeded = Vec::new();
    let mut failed = Vec::new();

    while let Some(join_res) = tasks.join_next().await {
        if let Ok((path, del_res)) = join_res {
            match del_res {
                Ok(_) => succeeded.push(path),
                Err(e) => failed.push(DeleteResultItem {
                    path,
                    error: e.to_string(),
                }),
            }
        }
    }

    // Sort outputs for determinism
    succeeded.sort();
    failed.sort_by(|a, b| a.path.cmp(&b.path));

    let success = failed.is_empty();
    let message = if success {
        format!("Deleted {} item(s)", succeeded.len())
    } else {
        format!(
            "Deleted {} item(s), {} failed",
            succeeded.len(),
            failed.len()
        )
    };

    Ok(Json(DeleteResponse {
        success,
        succeeded,
        failed,
        message,
    }))
}

/// Rename an entry
pub async fn rename_entry(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(connection_id): Path<String>,
    Json(payload): Json<TransferRequest>,
) -> Result<impl IntoResponse, AppError> {
    check_permission(&state.db, &user, &connection_id, PermissionAction::Write).await?;

    let provider = state.get_provider(&connection_id).await.ok_or_else(|| {
        VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
    })?;

    let from_vfs = VfsPath::new(&connection_id, &payload.from)?;
    let to_vfs = VfsPath::new(&connection_id, &payload.to)?;

    provider.rename(&from_vfs, &to_vfs).await?;

    crate::auth::record_audit_log(
        &state.db,
        Some(&user.id),
        "FILE_RENAME",
        Some(&connection_id),
        Some(&from_vfs.path),
        "SUCCESS",
        None,
        Some(&format!("Renamed {} -> {}", from_vfs.path, to_vfs.path)),
    )
    .await;

    Ok(Json(SuccessResponse {
        success: true,
        message: format!("Renamed to: {}", to_vfs.path),
    }))
}

/// Copy an entry
pub async fn copy_entry(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(connection_id): Path<String>,
    Json(payload): Json<TransferRequest>,
) -> Result<impl IntoResponse, AppError> {
    check_permission(&state.db, &user, &connection_id, PermissionAction::Create).await?;

    let provider = state.get_provider(&connection_id).await.ok_or_else(|| {
        VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
    })?;

    let from_vfs = VfsPath::new(&connection_id, &payload.from)?;
    let to_vfs = VfsPath::new(&connection_id, &payload.to)?;

    provider.copy(&from_vfs, &to_vfs).await?;

    crate::auth::record_audit_log(
        &state.db,
        Some(&user.id),
        "FILE_COPY",
        Some(&connection_id),
        Some(&from_vfs.path),
        "SUCCESS",
        None,
        Some(&format!("Copied {} -> {}", from_vfs.path, to_vfs.path)),
    )
    .await;

    Ok(Json(SuccessResponse {
        success: true,
        message: format!("Copied to: {}", to_vfs.path),
    }))
}

/// Streaming multipart upload with atomic .part staging
pub async fn upload_file(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(connection_id): Path<String>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, AppError> {
    check_permission(&state.db, &user, &connection_id, PermissionAction::Upload).await?;
    let provider = state.get_provider(&connection_id).await.ok_or_else(|| {
        VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
    })?;

    let mut dest_dir = "/".to_string();
    let mut uploaded_files = Vec::new();

    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Multipart parse error: {}", e)))?
    {
        let name = field.name().unwrap_or_default().to_string();

        if name == "path" {
            dest_dir = field.text().await.unwrap_or_else(|_| "/".to_string());
            continue;
        }

        if let Some(file_name) = field.file_name() {
            let clean_name = file_name.to_string();
            let target_path = VfsPath::new(
                &connection_id,
                format!("{}/{}", dest_dir.trim_end_matches('/'), clean_name),
            )?;
            let part_path =
                VfsPath::new(&connection_id, format!("{}.aerofs.part", target_path.path))?;

            let target_perms = crate::domain::resolve_destination_permissions(
                &provider,
                &target_path,
                false,
                crate::domain::PermissionInheritanceMode::InheritExistingOrParent,
            )
            .await;

            // Free-space preflight: check host disk capacity for local storage
            if connection_id == "local" {
                if let Some(free_bytes) =
                    get_available_disk_space(&state.config.filesystem.default_local_root)
                {
                    if free_bytes < 10 * 1024 * 1024 {
                        return Err(AppError::InsufficientStorage(format!(
                            "Local filesystem storage full: only {} MB free",
                            free_bytes / (1024 * 1024)
                        )));
                    }
                }
            }

            // Bounded 64 KiB asynchronous duplex pipe with zero whole-file RAM buffering
            let (duplex_reader, mut duplex_writer) = tokio::io::duplex(64 * 1024);
            let write_handle = tokio::spawn({
                let provider = provider.clone();
                let part_path = part_path.clone();
                async move {
                    provider
                        .write_stream(&part_path, Box::new(duplex_reader))
                        .await
                }
            });

            use tokio::io::AsyncWriteExt;
            let max_upload_bytes = state.config.limits.max_upload_size;
            let mut uploaded_bytes = 0u64;
            let mut stream_err = None;
            while let Some(chunk) = match field.chunk().await {
                Ok(c) => c,
                Err(e) => {
                    stream_err = Some(AppError::BadRequest(format!("Upload stream error: {}", e)));
                    None
                }
            } {
                uploaded_bytes += chunk.len() as u64;
                if uploaded_bytes > max_upload_bytes {
                    stream_err = Some(AppError::PayloadTooLarge(format!(
                        "Uploaded file exceeded maximum upload size limit of {} bytes",
                        max_upload_bytes
                    )));
                    break;
                }
                if let Err(e) = duplex_writer.write_all(&chunk).await {
                    stream_err = Some(AppError::Internal(anyhow::anyhow!(
                        "Failed writing upload chunk: {}",
                        e
                    )));
                    break;
                }
            }
            drop(duplex_writer);

            if let Some(err) = stream_err {
                let _ = write_handle.await;
                let _ = provider.delete(&part_path).await;
                return Err(err);
            }

            let write_res = write_handle.await.map_err(|e| {
                AppError::Internal(anyhow::anyhow!("Upload worker task error: {}", e))
            })?;

            if let Err(e) = write_res {
                let _ = provider.delete(&part_path).await;
                return Err(AppError::from(e));
            }

            // If target already existed or parent perms found, apply to .part before promoting
            if let Some(ref perms) = target_perms {
                let _ = provider.set_permissions(&part_path, perms).await;
            }

            // Stream completed cleanly -> promote .aerofs.part to final target
            if let Err(rename_err) = provider.rename(&part_path, &target_path).await {
                // If direct rename fails, attempt fallback copy + cleanup
                if let Err(copy_err) = provider.copy(&part_path, &target_path).await {
                    let _ = provider.delete(&part_path).await;
                    return Err(AppError::Internal(anyhow::anyhow!(
                        "Failed finalizing uploaded file: rename error ({}), copy error ({})",
                        rename_err,
                        copy_err
                    )));
                }
                let _ = provider.delete(&part_path).await;
            }

            if let Some(ref perms) = target_perms {
                let _ = provider.set_permissions(&target_path, perms).await;
            }

            crate::auth::record_audit_log(
                &state.db,
                Some(&user.id),
                "FILE_UPLOAD",
                Some(&connection_id),
                Some(&target_path.path),
                "SUCCESS",
                None,
                Some(&format!("Uploaded: {}", target_path.path)),
            )
            .await;

            uploaded_files.push(target_path.path);
        }
    }

    Ok(Json(SuccessResponse {
        success: true,
        message: format!("Uploaded {} file(s)", uploaded_files.len()),
    }))
}

/// Change permissions (CHMOD) of a file or directory
pub async fn chmod_file(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(connection_id): Path<String>,
    Json(payload): Json<ChmodRequest>,
) -> Result<impl IntoResponse, AppError> {
    check_permission(&state.db, &user, &connection_id, PermissionAction::Write).await?;

    let provider = state.get_provider(&connection_id).await.ok_or_else(|| {
        VfsError::ConnectionError(format!("Connection '{}' not found", connection_id))
    })?;

    let vfs_path = VfsPath::new(&connection_id, &payload.path)?;

    let formatted_mode = format!("{:04o}", payload.mode);
    provider.set_permissions(&vfs_path, &formatted_mode).await?;

    #[cfg(unix)]
    {
        if payload.recursive.unwrap_or(false) {
            let root = if let Some(custom) = state.get_system_setting("local_root").await {
                std::path::PathBuf::from(custom)
            } else {
                state.config.filesystem.default_local_root.clone()
            };
            if let Ok(safe_path) = crate::filesystem::safepath::SafePath::resolve(
                &root,
                &payload.path,
                state.config.security.allow_symlinks_outside_root,
            ) {
                let abs_path = safe_path.absolute();
                if abs_path.is_dir() {
                    let (succeeded, failed) = apply_chmod_recursive(abs_path, payload.mode).await;
                    if !failed.is_empty() {
                        return Ok(Json(serde_json::json!({
                            "success": false,
                            "succeeded": succeeded,
                            "failed": failed,
                            "message": format!("Chmod partially completed: {} succeeded, {} failed", succeeded, failed.len())
                        })));
                    }
                }
            }
        }
    }

    #[cfg(not(unix))]
    {
        return Err(AppError::BadRequest(
            "CHMOD is only supported on Unix systems".into(),
        ));
    }

    crate::auth::record_audit_log(
        &state.db,
        Some(&user.id),
        "FILE_CHMOD",
        Some(&connection_id),
        Some(&vfs_path.path),
        "SUCCESS",
        None,
        Some(&format!(
            "Changed permissions to {:o} on {}",
            payload.mode, vfs_path.path
        )),
    )
    .await;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("Permissions updated for {}", payload.path)
    })))
}

#[cfg(unix)]
async fn apply_chmod_recursive(dir: &std::path::Path, mode: u32) -> (usize, Vec<String>) {
    use std::os::unix::fs::PermissionsExt;
    let mut succeeded = 0;
    let mut failed = Vec::new();
    let mut stack = vec![dir.to_path_buf()];

    while let Some(curr_dir) = stack.pop() {
        if let Ok(mut entries) = tokio::fs::read_dir(&curr_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                let perms = std::fs::Permissions::from_mode(mode);
                match std::fs::set_permissions(&path, perms) {
                    Ok(_) => succeeded += 1,
                    Err(e) => failed.push(format!("{}: {}", path.display(), e)),
                }
                if path.is_dir() {
                    stack.push(path);
                }
            }
        }
    }
    (succeeded, failed)
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct StorageInfoResponse {
    pub source_name: String,
    pub source_size_formatted: String,
    pub disk_label: String,
    pub disk_usage_text: String,
    pub used_percent: u8,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub free_bytes: u64,
}

/// Get real disk and storage statistics for a connection (Zero-blocking via statvfs)
pub async fn get_storage_info(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(connection_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    if connection_id == "local" {
        let root = if let Some(custom) = state.get_system_setting("local_root").await {
            std::path::PathBuf::from(custom)
        } else {
            state.config.filesystem.default_local_root.clone()
        };

        #[cfg(unix)]
        {
            let mut stat = std::mem::MaybeUninit::uninit();
            if let Ok(c_path) = std::ffi::CString::new(root.to_string_lossy().as_bytes()) {
                if unsafe { libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) } == 0 {
                    let stat = unsafe { stat.assume_init() };
                    let total = stat.f_blocks * stat.f_frsize;
                    let free = stat.f_bavail * stat.f_frsize;
                    let used = total.saturating_sub(free);
                    let pct = if total > 0 {
                        ((used as f64 / total as f64) * 100.0) as u8
                    } else {
                        0
                    };

                    let total_gib = (total as f64) / (1024.0 * 1024.0 * 1024.0);

                    return Ok(Json(StorageInfoResponse {
                        source_name: "Local Storage".to_string(),
                        source_size_formatted: format_bytes_str(used),
                        disk_label: "Disk".to_string(),
                        disk_usage_text: format!("{}% · {:.0} GiB", pct, total_gib),
                        used_percent: pct,
                        total_bytes: total,
                        used_bytes: used,
                        free_bytes: free,
                    }));
                }
            }
        }

        return Ok(Json(StorageInfoResponse {
            source_name: "Local Storage".to_string(),
            source_size_formatted: "Local".to_string(),
            disk_label: "Disk".to_string(),
            disk_usage_text: "Available".to_string(),
            used_percent: 0,
            total_bytes: 0,
            used_bytes: 0,
            free_bytes: 0,
        }));
    }

    // For Remote / FTP connections
    let row: Option<(String, String, Option<String>, Option<i64>)> =
        sqlx::query_as("SELECT name, provider, host, port FROM connections WHERE id = ?")
            .bind(&connection_id)
            .fetch_optional(&state.db)
            .await
            .unwrap_or(None);

    if let Some((name, provider, host, port)) = row {
        let port_str = port.map(|p| p.to_string()).unwrap_or_else(|| "21".into());
        let host_str = host.unwrap_or_else(|| "Remote".into());
        return Ok(Json(StorageInfoResponse {
            source_name: name,
            source_size_formatted: format!("{} Remote", provider.to_uppercase()),
            disk_label: format!("{}:{}", host_str, port_str),
            disk_usage_text: "Connected · Online".to_string(),
            used_percent: 0,
            total_bytes: 0,
            used_bytes: 0,
            free_bytes: 0,
        }));
    }

    Ok(Json(StorageInfoResponse {
        source_name: connection_id,
        source_size_formatted: "Remote".to_string(),
        disk_label: "Network".to_string(),
        disk_usage_text: "Connected".to_string(),
        used_percent: 0,
        total_bytes: 0,
        used_bytes: 0,
        free_bytes: 0,
    }))
}

fn format_bytes_str(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 * 1024 {
        format!(
            "{:.1} TiB",
            bytes as f64 / (1024.0 * 1024.0 * 1024.0 * 1024.0)
        )
    } else if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1} GiB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}
