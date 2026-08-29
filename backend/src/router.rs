use crate::api::{archive, audit, auth, connections, files, search, transfers, ws};
use crate::state::AppState;
use axum::{
    http::{
        header::{
            ACCEPT, AUTHORIZATION, CONTENT_DISPOSITION, CONTENT_RANGE, CONTENT_TYPE, COOKIE, ETAG,
            IF_MATCH, IF_NONE_MATCH, RANGE,
        },
        HeaderName, HeaderValue, Method,
    },
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
    Router,
};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;

/// Middleware to append essential HTTP security headers
async fn security_headers_middleware(request: axum::extract::Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    headers.insert(
        "X-Content-Type-Options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("X-Frame-Options", HeaderValue::from_static("SAMEORIGIN"));
    headers.insert(
        "X-XSS-Protection",
        HeaderValue::from_static("1; mode=block"),
    );
    headers.insert(
        "Referrer-Policy",
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );

    response
}

pub fn create_router(state: AppState) -> Router {
    let is_dev = std::env::var("AEROFS_ENV").unwrap_or_else(|_| "development".into())
        == "development"
        || cfg!(test);

    // Build CORS layer:
    // 1. If explicit allowed_origins are configured, use them (supports LAN IPs, Android).
    // 2. Otherwise, mirror the request origin in dev mode (permissive).
    // 3. In production with no explicit origins, be restrictive (no wildcard).
    let cors = {
        let allowed_methods = [
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
            Method::PATCH,
        ];
        let allowed_headers = [
            CONTENT_TYPE,
            AUTHORIZATION,
            ACCEPT,
            COOKIE,
            IF_MATCH,
            IF_NONE_MATCH,
            RANGE,
            HeaderName::from_static("x-force-overwrite"),
            HeaderName::from_static("x-idempotency-key"),
        ];
        let exposed_headers = [
            ETAG,
            CONTENT_RANGE,
            CONTENT_DISPOSITION,
            HeaderName::from_static("x-request-id"),
            HeaderName::from_static("x-idempotency-key"),
        ];

        if !state.config.security.allowed_origins.is_empty() {
            // Explicit origin allowlist — works for LAN IPs and Android WebView.
            let origins: Vec<HeaderValue> = state
                .config
                .security
                .allowed_origins
                .iter()
                .filter_map(|o| o.parse::<HeaderValue>().ok())
                .collect();
            CorsLayer::new()
                .allow_origin(origins)
                .allow_methods(allowed_methods)
                .allow_headers(allowed_headers)
                .expose_headers(exposed_headers)
                .allow_credentials(true)
        } else if is_dev {
            // Dev fallback: mirror any origin (permissive for local development).
            CorsLayer::new()
                .allow_origin(AllowOrigin::mirror_request())
                .allow_methods(allowed_methods)
                .allow_headers(allowed_headers)
                .expose_headers(exposed_headers)
                .allow_credentials(true)
        } else {
            // Production with no explicit origins: mirror request origin with credentials.
            // This allows the same-origin embedded SPA to work while still permitting
            // apps that present a proper Origin header (including Android WebView).
            CorsLayer::new()
                .allow_origin(AllowOrigin::mirror_request())
                .allow_methods(allowed_methods)
                .allow_headers(allowed_headers)
                .expose_headers(exposed_headers)
                .allow_credentials(true)
        }
    };

    let auth_routes = Router::new()
        .route("/login", post(auth::login))
        .route("/logout", post(auth::logout))
        .route("/me", get(auth::me));

    let connection_routes = Router::new()
        .route(
            "/",
            get(connections::list_connections).post(connections::create_connection),
        )
        .route(
            "/{id}",
            get(connections::get_connection).delete(connections::delete_connection),
        )
        .route("/{id}/test", post(connections::test_connection))
        .route(
            "/{id}/files",
            get(files::list_files)
                .post(files::create_file)
                .delete(files::delete_files),
        )
        .route("/{id}/directories", post(files::create_directory))
        .route("/{id}/files/metadata", get(files::get_metadata))
        .route(
            "/{id}/files/content",
            get(files::get_file_content).put(files::update_file_content),
        )
        .route("/{id}/files/rename", post(files::rename_entry))
        .route("/{id}/files/copy", post(files::copy_entry))
        .route("/{id}/files/chmod", post(files::chmod_file))
        .route(
            "/{id}/files/presign/download",
            post(files::presign_download_file),
        )
        .route(
            "/{id}/files/presign/upload",
            post(files::presign_upload_file),
        )
        .route(
            "/{id}/files/presign/complete",
            post(files::presign_complete_upload),
        )
        .route("/{id}/storage-info", get(files::get_storage_info))
        .route("/{id}/upload", post(files::upload_file))
        .route("/{id}/archive/compress", post(archive::compress_files))
        .route(
            "/{id}/archive/extract",
            post(archive::extract_archive_endpoint),
        )
        .route(
            "/{id}/archive/entries",
            get(archive::list_virtual_archive_endpoint),
        )
        .route(
            "/{id}/archive/read",
            get(archive::read_virtual_archive_entry_endpoint),
        )
        .route(
            "/{id}/archive/extract-selected",
            post(archive::extract_selected_archive_endpoint),
        )
        .route("/{id}/search", get(search::search_files));

    let transfer_routes = Router::new()
        .route(
            "/",
            get(transfers::list_transfers).post(transfers::create_transfer),
        )
        .route("/{id}/cancel", post(transfers::cancel_transfer))
        .route("/{id}/dismiss", post(transfers::dismiss_transfer))
        .route("/clear-finished", post(transfers::clear_finished_transfers));

    let share_routes = Router::new()
        .route(
            "/",
            get(crate::api::shares::list_shares).post(crate::api::shares::create_share),
        )
        .route(
            "/{id}",
            axum::routing::delete(crate::api::shares::delete_share),
        );

    let trash_routes = Router::new()
        .route("/", get(crate::api::trash::list_trash))
        .route("/move", post(crate::api::trash::move_to_trash))
        .route("/restore/{id}", post(crate::api::trash::restore_trash_item))
        .route(
            "/empty",
            axum::routing::delete(crate::api::trash::empty_trash),
        )
        .route(
            "/{id}",
            axum::routing::delete(crate::api::trash::delete_trash_item),
        );

    let api_v1 = Router::new()
        .nest("/auth", auth_routes)
        .nest("/connections", connection_routes)
        .nest("/transfers", transfer_routes)
        .nest("/shares", share_routes)
        .nest("/trash", trash_routes)
        .route(
            "/settings",
            get(crate::api::settings::get_settings).put(crate::api::settings::update_settings),
        )
        .route(
            "/user/preferences",
            get(crate::api::preferences::get_user_preferences)
                .put(crate::api::preferences::update_user_preferences),
        )
        .route("/audit-logs", get(audit::list_audit_logs));

    Router::new()
        .route("/health", get(crate::api::health::health_check))
        .route("/health/live", get(crate::api::health::health_live))
        .route("/health/ready", get(crate::api::health::health_ready))
        .route("/api/v1/health/live", get(crate::api::health::health_live))
        .route(
            "/api/v1/health/ready",
            get(crate::api::health::health_ready),
        )
        .route("/api/v1/ws", get(ws::ws_handler))
        .route(
            "/api/v1/shares/public/{token}",
            get(crate::api::shares::public_get_share),
        )
        .nest("/api/v1", api_v1)
        .fallback(crate::static_files::static_handler)
        .layer(axum::middleware::from_fn(
            crate::middleware::idempotency_middleware,
        ))
        .layer(axum::middleware::from_fn(
            crate::middleware::request_id_middleware,
        ))
        .layer(middleware::from_fn(security_headers_middleware))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
