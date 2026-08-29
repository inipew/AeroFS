use axum::{routing::get, Json, Router};
use utoipa::OpenApi;

use crate::api::files::{
    ChmodRequest, CreateEntryRequest, DeleteRequest, PresignRequest, PresignResponse,
    SuccessResponse, TransferRequest, UpdateContentRequest,
};
use crate::domain::{
    capabilities::{Capabilities, ChecksumCapabilities},
    file::{DirectoryListing, FileEntry, FileKind, FileMetadata, FileVersion},
};
use crate::state::AppState;

/// AeroFS OpenAPI 3.1 document, generated automatically from Rust types
/// annotated with `#[derive(utoipa::ToSchema)]`.
///
/// Note: Query parameter structs that use `IntoParams` (not `ToSchema`) are
/// intentionally excluded from the `schemas` list — they appear only in
/// `parameters` within individual path items.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "AeroFS API",
        version = "1.0.0",
        description = "Remote file manager REST API. All endpoints require session authentication except public share endpoints.",
        license(name = "Proprietary")
    ),
    components(schemas(
        // Domain types
        FileKind,
        FileEntry,
        FileMetadata,
        FileVersion,
        DirectoryListing,
        Capabilities,
        ChecksumCapabilities,
        // Request / Response body schemas
        PresignRequest,
        PresignResponse,
        CreateEntryRequest,
        UpdateContentRequest,
        DeleteRequest,
        ChmodRequest,
        TransferRequest,
        SuccessResponse,
    )),
    tags(
        (name = "files", description = "File and directory operations"),
        (name = "connections", description = "Storage connection management"),
        (name = "transfers", description = "Background transfer jobs"),
        (name = "auth", description = "Authentication"),
        (name = "shares", description = "Public share links"),
    )
)]
pub struct ApiDoc;

/// `GET /openapi.json` — serve the OpenAPI 3.1 schema.
///
/// In production this endpoint should be behind an auth guard or disabled.
/// For now it is open to allow `openapi-typescript` to generate frontend types.
pub async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

/// Mount the OpenAPI endpoint.
pub fn openapi_router() -> Router<AppState> {
    Router::new().route("/openapi.json", get(openapi_json))
}
