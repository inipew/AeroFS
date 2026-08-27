use axum::{
    http::{header, HeaderValue, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../frontend/dist"]
pub struct FrontendAssets;

/// Handler for serving embedded frontend static files with SPA fallback
pub async fn static_handler(uri: Uri) -> Response {
    let mut path = uri.path().trim_start_matches('/').to_string();
    if path.is_empty() {
        path = "index.html".to_string();
    }

    match FrontendAssets::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            let mut res = (
                StatusCode::OK,
                [(
                    header::CONTENT_TYPE,
                    HeaderValue::from_str(mime.as_ref())
                        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
                )],
                content.data,
            )
                .into_response();

            // Cache-Control headers: long cache for hashed assets, no-cache for index.html
            let headers = res.headers_mut();
            if path.starts_with("assets/") {
                headers.insert(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("public, max-age=31536000, immutable"),
                );
            } else {
                headers.insert(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("no-cache, must-revalidate"),
                );
            }

            res
        }
        None => {
            // If the specific file wasn't found (e.g. /settings, /login), fallback to index.html for Vue Router SPA
            match FrontendAssets::get("index.html") {
                Some(index) => {
                    let mut res = (
                        StatusCode::OK,
                        [(
                            header::CONTENT_TYPE,
                            HeaderValue::from_static("text/html; charset=utf-8"),
                        )],
                        index.data,
                    )
                        .into_response();

                    res.headers_mut().insert(
                        header::CACHE_CONTROL,
                        HeaderValue::from_static("no-cache, must-revalidate"),
                    );
                    res
                }
                None => (
                    StatusCode::NOT_FOUND,
                    "AeroFS: Embedded frontend assets not found. Run `npm run build` in the frontend directory before building the release binary.",
                )
                    .into_response(),
            }
        }
    }
}
