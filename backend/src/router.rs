use crate::api::{archive, audit, auth, connections, files, health_check, search, transfers, ws};
use crate::state::AppState;
use axum::{
    http::{
        header::{
            ACCEPT, AUTHORIZATION, CONTENT_TYPE, COOKIE,
        },
        HeaderValue, Method,
    },
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
    Router,
};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;

/// Middleware to append essential HTTP security headers
async fn security_headers_middleware(
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    headers.insert(
        "X-Content-Type-Options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        "X-Frame-Options",
        HeaderValue::from_static("SAMEORIGIN"),
    );
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
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::mirror_request())
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
            Method::PATCH,
        ])
        .allow_headers([CONTENT_TYPE, AUTHORIZATION, ACCEPT, COOKIE])
        .allow_credentials(true);

    let auth_routes = Router::new()
        .route("/login", post(auth::login))
        .route("/logout", post(auth::logout))
        .route("/me", get(auth::me));

    let connection_routes = Router::new()
        .route("/", get(connections::list_connections).post(connections::create_connection))
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
        .route("/{id}/files/move", post(files::rename_entry))
        .route("/{id}/files/chmod", post(files::chmod_file))
        .route("/{id}/storage-info", get(files::get_storage_info))
        .route("/{id}/upload", post(files::upload_file))
        .route("/{id}/archive/compress", post(archive::compress_files))
        .route("/{id}/archive/extract", post(archive::extract_archive_endpoint))
        .route("/{id}/search", get(search::search_files));

    let transfer_routes = Router::new()
        .route("/", get(transfers::list_transfers).post(transfers::create_transfer))
        .route("/{id}/cancel", post(transfers::cancel_transfer));

    let share_routes = Router::new()
        .route("/", get(crate::api::shares::list_shares).post(crate::api::shares::create_share))
        .route("/{id}", axum::routing::delete(crate::api::shares::delete_share));

    let trash_routes = Router::new()
        .route("/", get(crate::api::trash::list_trash))
        .route("/move", post(crate::api::trash::move_to_trash))
        .route("/restore/{id}", post(crate::api::trash::restore_trash_item))
        .route("/empty", axum::routing::delete(crate::api::trash::empty_trash))
        .route("/{id}", axum::routing::delete(crate::api::trash::delete_trash_item));

    let api_v1 = Router::new()
        .nest("/auth", auth_routes)
        .nest("/connections", connection_routes)
        .nest("/transfers", transfer_routes)
        .nest("/shares", share_routes)
        .nest("/trash", trash_routes)
        .route("/settings", get(crate::api::settings::get_settings).put(crate::api::settings::update_settings))
        .route("/audit-logs", get(audit::list_audit_logs));

    Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/ws", get(ws::ws_handler))
        .route("/api/v1/shares/public/{token}", get(crate::api::shares::public_get_share))
        .nest("/api/v1", api_v1)
        .layer(middleware::from_fn(security_headers_middleware))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
