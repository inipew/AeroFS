use crate::api::{
    archive as api_archive, audit, connections as api_connections, files as api_files, openapi,
    search as api_search, ws,
};
use crate::state::{AppState, RouterState, RuntimeState};

pub mod auth;
pub mod files;
pub mod transfers;
use axum::{
    extract::FromRef,
    http::{
        header::{
            ACCEPT, AUTHORIZATION, CONTENT_DISPOSITION, CONTENT_RANGE, CONTENT_TYPE, COOKIE, ETAG,
            IF_MATCH, IF_NONE_MATCH, RANGE,
        },
        HeaderName, HeaderValue, Method,
    },
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;

async fn security_headers_middleware(request: axum::extract::Request, next: Next) -> Response {
    let is_https = request
        .headers()
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .map(|v| v == "https")
        .unwrap_or(false);
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    headers.insert("X-Content-Type-Options", HeaderValue::from_static("nosniff"));
    headers.insert("X-Frame-Options", HeaderValue::from_static("SAMEORIGIN"));
    if !headers.contains_key("content-security-policy") {
        headers.insert(
            "Content-Security-Policy",
            HeaderValue::from_static("frame-ancestors 'self'; default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src 'self' ws: wss:"),
        );
    }
    headers.insert(
        "Referrer-Policy",
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        "Permissions-Policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );
    headers.insert(
        "Cross-Origin-Opener-Policy",
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(
        "Cross-Origin-Resource-Policy",
        HeaderValue::from_static("same-origin"),
    );
    if is_https {
        headers.insert(
            "Strict-Transport-Security",
            HeaderValue::from_static("max-age=63072000; includeSubDomains; preload"),
        );
    }
    response
}

async fn shutdown_guard(
    axum::extract::State(state): axum::extract::State<RuntimeState>,
    req: axum::extract::Request,
    next: Next,
) -> Response {
    if state.is_shutting_down() {
        let method = req.method().clone();
        let path = req.uri().path();
        let is_ws_upgrade = path == "/api/v1/ws";
        let is_readonly =
            matches!(method, Method::GET | Method::HEAD | Method::OPTIONS) && !is_ws_upgrade;
        let is_transfer_cancel = method == Method::POST
            && path.starts_with("/api/v1/transfers/")
            && path.ends_with("/cancel");
        let is_auth_logout = method == Method::POST && path == "/api/v1/auth/logout";

        if !is_readonly && !is_transfer_cancel && !is_auth_logout {
            tracing::debug!(
                "shutdown_guard: rejected {} {} (server shutting down)",
                method,
                path
            );
            let mut response = crate::errors::AppError::ServiceUnavailable(
                "Server is shutting down. Please retry after restart.".to_string(),
            )
            .into_response();
            response.headers_mut().insert(
                axum::http::header::RETRY_AFTER,
                axum::http::HeaderValue::from_static("5"),
            );
            return response;
        }
    }
    next.run(req).await
}

async fn api_error_response_middleware(req: axum::extract::Request, next: Next) -> Response {
    let path = req.uri().path().to_string();
    let resp = next.run(req).await;
    if (path.starts_with("/api/") || path.starts_with("/health"))
        && resp.status() == axum::http::StatusCode::METHOD_NOT_ALLOWED
    {
        let (parts, _) = resp.into_parts();
        let mut err_resp = crate::errors::AppError::MethodNotAllowed(
            "Method not allowed for this endpoint".to_string(),
        )
        .into_response();
        for (k, v) in parts.headers.iter() {
            if k == axum::http::header::ALLOW {
                err_resp.headers_mut().insert(k.clone(), v.clone());
            }
        }
        return err_resp;
    }
    resp
}

pub fn create_router(state: AppState) -> Router {
    let router_state = RouterState::from_ref(&state);
    let runtime_state = RuntimeState::from_ref(&state);

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
            HeaderName::from_static("idempotency-key"),
            HeaderName::from_static("x-cache-idempotency"),
        ];
        let exposed_headers = [
            ETAG,
            CONTENT_RANGE,
            CONTENT_DISPOSITION,
            HeaderName::from_static("x-request-id"),
            HeaderName::from_static("x-idempotency-key"),
            HeaderName::from_static("idempotency-key"),
            HeaderName::from_static("x-cache-idempotency"),
            HeaderName::from_static("x-next-cursor"),
        ];

        if !router_state.allowed_origins.is_empty() {
            let origins: Vec<HeaderValue> = router_state
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
        } else if router_state.is_dev {
            CorsLayer::new()
                .allow_origin(AllowOrigin::mirror_request())
                .allow_methods(allowed_methods)
                .allow_headers(allowed_headers)
                .expose_headers(exposed_headers)
                .allow_credentials(true)
        } else {
            CorsLayer::new()
                .allow_origin(AllowOrigin::predicate(|_origin: &HeaderValue, _| false))
                .allow_methods(allowed_methods)
                .allow_headers(allowed_headers)
                .expose_headers(exposed_headers)
                .allow_credentials(true)
        }
    };

    let auth_routes = self::auth::router();

    let connection_routes = Router::new()
        .route(
            "/",
            get(api_connections::list_connections).post(api_connections::create_connection),
        )
        .route(
            "/{id}",
            get(api_connections::get_connection)
                .put(api_connections::update_connection)
                .delete(api_connections::delete_connection),
        )
        .route("/{id}/test", post(api_connections::test_connection))
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
        .route("/{id}/files/presign/download", post(api_files::presign_download_file))
        .route("/{id}/files/presign/upload", post(api_files::presign_upload_file));

    let router = Router::new()
        .nest("/api/v1/auth", auth_routes)
        .nest("/api/v1/connections", connection_routes)
        .nest("/api/v1/transfers", self::transfers::router())
        .nest("/api/v1", self::files::router())
        .route("/api/v1/search", get(api_search::search))
        .route("/api/v1/ws", get(ws::websocket_handler))
        .route("/api/v1/audit", get(audit::list_audit_logs))
        .route("/api/v1/openapi.json", get(openapi::openapi_json))
        .route("/metrics", get(crate::api::health::runtime_metrics))
        .merge(crate::api::router::create_api_router())
        .layer(middleware::from_fn(api_error_response_middleware))
        .layer(middleware::from_fn_with_state(
            runtime_state,
            shutdown_guard,
        ))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .layer(middleware::from_fn(security_headers_middleware))
        .with_state(state);

    router
}
