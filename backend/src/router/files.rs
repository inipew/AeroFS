//! Files router sub-module (§45) — extracted from router.rs 978L monolith

use crate::api::{archive as api_archive, files as api_files, search as api_search};
use crate::state::AppState;
use axum::routing::{get, post};
use axum::Router;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/{id}/files",
            get(api_files::list_files)
                .post(api_files::create_file)
                .delete(api_files::delete_files),
        )
        .route("/{id}/directories", post(api_files::create_directory))
        .route("/{id}/files/metadata", get(api_files::get_metadata))
        .route(
            "/{id}/files/content",
            get(api_files::get_file_content).put(api_files::update_file_content),
        )
        .route("/{id}/files/rename", post(api_files::rename_entry))
        .route("/{id}/files/copy", post(api_files::copy_entry))
        .route("/{id}/files/chmod", post(api_files::chmod_file))
        .route(
            "/{id}/files/presign/download",
            post(api_files::presign_download_file),
        )
        .route(
            "/{id}/files/presign/upload",
            post(api_files::presign_upload_file),
        )
        .route(
            "/{id}/files/presign/complete",
            post(api_files::presign_complete_upload),
        )
        .route("/{id}/storage-info", get(api_files::get_storage_info))
        .route("/{id}/upload", post(api_files::upload_file))
        .route("/{id}/archive/compress", post(api_archive::compress_files))
        .route(
            "/{id}/archive/extract",
            post(api_archive::extract_archive_endpoint),
        )
        .route(
            "/{id}/archive/entries",
            get(api_archive::list_virtual_archive_endpoint),
        )
        .route(
            "/{id}/archive/read",
            get(api_archive::read_virtual_archive_entry_endpoint),
        )
        .route(
            "/{id}/archive/extract-selected",
            post(api_archive::extract_selected_archive_endpoint),
        )
        .route("/{id}/search", get(api_search::search_files))
}
