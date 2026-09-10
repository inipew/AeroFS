use axum::{routing::get, Json, Router};
use utoipa::OpenApi;

use crate::errors::{ErrorCategory, ErrorCode, ErrorDetail, ErrorResponse};
use crate::state::AppState;

struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "CookieAuth",
                utoipa::openapi::security::SecurityScheme::ApiKey(
                    utoipa::openapi::security::ApiKey::Cookie(
                        utoipa::openapi::security::ApiKeyValue::new("aerofs_session"),
                    ),
                ),
            );
            components.add_security_scheme(
                "BearerAuth",
                utoipa::openapi::security::SecurityScheme::Http(
                    utoipa::openapi::security::HttpBuilder::new()
                        .scheme(utoipa::openapi::security::HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build(),
                ),
            );
        }
    }
}

/// AeroFS OpenAPI 3.1 document, generated automatically from Rust types
#[derive(OpenApi)]
#[openapi(
    info(
        title = "AeroFS API",
        version = "1.0.0",
        description = "Remote file manager REST API. All endpoints require session authentication except public share endpoints.",
        license(name = "Proprietary")
    ),
    paths(
        // Health
        crate::api::health::health_live,
        crate::api::health::health_ready,
        crate::api::health::health_check,
        // Auth
        crate::api::auth::login,
        crate::api::auth::logout,
        crate::api::auth::me,
        // Connections
        crate::api::connections::list_connections,
        crate::api::connections::create_connection,
        crate::api::connections::get_connection,
        crate::api::connections::update_connection,
        crate::api::connections::delete_connection,
        crate::api::connections::test_connection,
        // Files
        crate::api::files::list_files,
        crate::api::files::create_file,
        crate::api::files::delete_files,
        crate::api::files::create_directory,
        crate::api::files::stat_file,
        crate::api::files::get_file_content,
        crate::api::files::update_file_content,
        crate::api::files::rename_entry,
        crate::api::files::copy_entry,
        crate::api::files::chmod_file,
        crate::api::files::presign_download_file,
        crate::api::files::presign_upload_file,
        crate::api::files::presign_complete_upload,
        crate::api::files::get_storage_info,
        crate::api::files::create_upload_session,
        crate::api::files::upload_session_content,
        crate::api::files::upload_file,
        // Archive
        crate::api::archive::compress_files,
        crate::api::archive::extract_archive_endpoint,
        crate::api::archive::list_virtual_archive_endpoint,
        crate::api::archive::read_virtual_archive_entry_endpoint,
        crate::api::archive::extract_selected_archive_endpoint,
        // Search
        crate::api::search::search_files,
        // Transfers
        crate::api::transfers::create_transfer,
        crate::api::transfers::list_transfers,
        crate::api::transfers::cancel_transfer,
        crate::api::transfers::retry_transfer,
        crate::api::transfers::dismiss_transfer,
        crate::api::transfers::clear_finished_transfers,
        // Sync
        crate::api::sync::create_sync_job,
        crate::api::sync::list_sync_jobs,
        crate::api::sync::list_operations,
        crate::api::sync::resolve_conflict,
        // Shares
        crate::api::shares::list_shares,
        crate::api::shares::create_share,
        crate::api::shares::delete_share,
        crate::api::shares::public_get_share,
        // Trash
        crate::api::trash::list_trash,
        crate::api::trash::move_to_trash,
        crate::api::trash::restore_trash_item,
        crate::api::trash::delete_trash_item,
        crate::api::trash::empty_trash,
        // Preferences
        crate::api::preferences::get_user_preferences,
        crate::api::preferences::update_user_preferences,
        // Settings
        crate::api::settings::get_settings,
        crate::api::settings::update_settings,
        // Audit
        crate::api::audit::list_audit_logs,
    ),
    components(schemas(
        // Common
        ErrorCode,
        ErrorCategory,
        ErrorDetail,
        ErrorResponse,
        // Health
        crate::api::health::LivenessResponse,
        crate::api::health::ReadinessResponse,
        // Auth
        crate::auth::session::UserInfo,
        crate::api::auth::LoginRequest,
        crate::api::auth::AuthResponse,
        crate::api::auth::LogoutResponse,
        // Connections
        crate::domain::connection::Connection,
        crate::domain::connection::ProviderKind,
        crate::domain::connection::ConnectionStatus,
        crate::services::connection_service::CreateConnectionRequest,
        crate::services::connection_service::UpdateConnectionRequest,
        crate::services::connection_service::ConnectionDetailResponse,
        crate::services::connection_service::TestConnectionResponse,
        crate::api::connections::CreateConnectionResponse,
        crate::api::connections::ConnectionActionResponse,
        // Files
        crate::domain::ids::SortField,
        crate::domain::ids::SortOrder,
        crate::domain::file::FileKind,
        crate::domain::file::FileEntry,
        crate::domain::file::FileMetadata,
        crate::domain::file::FileVersion,
        crate::domain::file::DirectoryListing,
        crate::domain::capabilities::Capabilities,
        crate::domain::capabilities::ChecksumCapabilities,
        crate::api::files::StorageInfoResponse,
        crate::api::files::CreateEntryRequest,
        crate::api::files::UpdateContentRequest,
        crate::api::files::DeleteRequest,
        crate::api::files::TransferRequest,
        crate::api::files::ChmodRequest,
        crate::api::files::ChmodResponse,
        crate::api::files::PresignRequest,
        crate::api::files::PresignResponse,
        crate::api::files::CreateUploadSessionRequest,
        crate::api::files::CreateUploadSessionResponse,
        crate::api::files::SuccessResponse,
        // Archive
        crate::filesystem::archive::ArchiveOverwriteMode,
        crate::filesystem::archive::VirtualArchiveEntry,
        crate::api::archive::CompressRequest,
        crate::api::archive::ExtractRequest,
        crate::api::archive::ExtractSelectedRequest,
        crate::api::archive::ArchiveResponse,
        // Search
        crate::filesystem::search::SearchOutput,
        crate::api::search::SearchQuery,
        // Transfers
        crate::transfer::TransferType,
        crate::transfer::TransferStatus,
        crate::transfer::TransferPhase,
        crate::transfer::TransferExecutionMode,
        crate::transfer::TransferStaging,
        crate::transfer::TransferJob,
        crate::api::transfers::CreateTransferRequest,
        crate::api::transfers::CreateTransferResponse,
        crate::api::transfers::TransferActionResponse,
        crate::api::transfers::ClearFinishedTransfersResponse,
        // Sync
        crate::sync::models::SyncStrategy,
        crate::sync::models::SyncStatus,
        crate::sync::models::SyncOpKind,
        crate::sync::models::FileManifest,
        crate::sync::models::SyncOperation,
        crate::sync::models::SyncJob,
        crate::api::sync::CreateSyncRequest,
        crate::api::sync::ResolveConflictRequest,
        crate::api::sync::CreateSyncResponse,
        crate::api::sync::ResolveConflictResponse,
        // Shares
        crate::services::share_service::ShareItem,
        crate::services::share_service::CreateShareRequest,
        crate::api::shares::ShareActionResponse,
        crate::api::shares::PublicShareQuery,
        // Trash
        crate::services::trash_service::TrashItem,
        crate::services::trash_service::MoveToTrashRequest,
        crate::services::trash_service::MovedTrashItem,
        crate::api::trash::MoveToTrashResponse,
        crate::api::trash::TrashActionResponse,
        crate::api::trash::EmptyTrashResponse,
        // Preferences
        crate::domain::settings::UserPreferences,
        crate::api::preferences::UpdatePreferencesResponse,
        // Settings
        crate::domain::settings::GeneralSettings,
        crate::domain::settings::FileManagerSettings,
        crate::domain::settings::TransferSettings,
        crate::domain::settings::ConnectionSettings,
        crate::domain::settings::SecuritySettings,
        crate::domain::settings::AdvancedSettings,
        crate::domain::settings::SystemSettings,
        crate::domain::settings::AppSettings,
        crate::services::settings_service::SettingsResponse,
        crate::services::settings_service::UpdateSettingsRequest,
        crate::api::settings::UpdateSettingsResponse,
        // Audit
        crate::auth::audit::AuditLogEntry,
        crate::api::audit::AuditLogQuery,
    )),
    modifiers(&SecurityAddon),
    tags(
        (name = "health", description = "Health checks"),
        (name = "auth", description = "Authentication"),
        (name = "connections", description = "Storage connections"),
        (name = "files", description = "File and directory operations"),
        (name = "archive", description = "Archive compress, extract, and inspection"),
        (name = "search", description = "File search"),
        (name = "transfers", description = "Background transfer jobs"),
        (name = "sync", description = "Folder synchronization"),
        (name = "shares", description = "Public share links"),
        (name = "trash", description = "Trash bin operations"),
        (name = "preferences", description = "User preferences"),
        (name = "settings", description = "System settings"),
        (name = "audit", description = "Audit logs"),
    )
)]
pub struct ApiDoc;

/// `GET /openapi.json` — serve the OpenAPI 3.1 schema.
pub async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

/// Mount the OpenAPI endpoint.
pub fn openapi_router() -> Router<AppState> {
    Router::new().route("/openapi.json", get(openapi_json))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openapi_spec_contains_all_sync_routes() {
        let openapi = ApiDoc::openapi();
        let paths = &openapi.paths.paths;
        assert!(paths.contains_key("/api/v1/sync"), "Missing /api/v1/sync path");
        assert!(paths.contains_key("/api/v1/sync/{id}/operations"), "Missing /api/v1/sync/{{id}}/operations path");
        assert!(paths.contains_key("/api/v1/sync/{id}/resolve"), "Missing /api/v1/sync/{{id}}/resolve path");

        let sync_path = paths.get("/api/v1/sync").expect("/api/v1/sync path item");
        assert!(sync_path.get.is_some(), "GET /api/v1/sync missing");
        assert!(sync_path.post.is_some(), "POST /api/v1/sync missing");

        let ops_path = paths.get("/api/v1/sync/{id}/operations").expect("/api/v1/sync/{id}/operations path item");
        assert!(ops_path.get.is_some(), "GET /api/v1/sync/{{id}}/operations missing");

        let resolve_path = paths.get("/api/v1/sync/{id}/resolve").expect("/api/v1/sync/{id}/resolve path item");
        assert!(resolve_path.post.is_some(), "POST /api/v1/sync/{{id}}/resolve missing");
    }
}
