use axum::{
    extract::Request,
    http::{header::HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

pub const REQUEST_ID_HEADER: &str = "x-request-id";

#[derive(Clone, Debug)]
pub struct RequestId(pub String);

/// Axum middleware for propagating and generating X-Request-ID headers across all requests
pub async fn request_id_middleware(mut req: Request, next: Next) -> Response {
    let req_id = req
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    req.extensions_mut().insert(RequestId(req_id.clone()));

    let mut response = next.run(req).await;

    if let Ok(val) = HeaderValue::from_str(&req_id) {
        response
            .headers_mut()
            .insert(HeaderName::from_static(REQUEST_ID_HEADER), val);
    }

    response
}
