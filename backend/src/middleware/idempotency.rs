use axum::{
    body::{to_bytes, Body},
    extract::Request,
    http::{header::HeaderName, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use axum::body::Bytes;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, RwLock};
use std::time::{Duration, Instant};

#[derive(Clone)]
struct CachedResponse {
    status: StatusCode,
    content_type: Option<HeaderValue>,
    body: Bytes,
    created_at: Instant,
}

static IDEMPOTENCY_CACHE: LazyLock<Arc<RwLock<HashMap<String, CachedResponse>>>> =
    LazyLock::new(|| Arc::new(RwLock::new(HashMap::new())));

const IDEMPOTENCY_HEADER: &str = "idempotency-key";
const CACHE_TTL: Duration = Duration::from_secs(300); // 5 minutes
const MAX_CACHE_ENTRIES: usize = 1000;

/// Axum middleware for transparent Idempotency-Key deduplication on mutating requests
pub async fn idempotency_middleware(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let is_mutating = method == Method::POST
        || method == Method::PUT
        || method == Method::PATCH
        || method == Method::DELETE;

    if !is_mutating {
        return next.run(req).await;
    }

    let key_opt = req
        .headers()
        .get(IDEMPOTENCY_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let key = match key_opt {
        Some(k) => k,
        None => return next.run(req).await,
    };

    // 1. Check existing cache
    if let Ok(guard) = IDEMPOTENCY_CACHE.read() {
        if let Some(cached) = guard.get(&key) {
            if cached.created_at.elapsed() < CACHE_TTL {
                tracing::debug!("Idempotency hit for key: {}", key);
                let mut resp = Response::builder().status(cached.status);
                if let Some(ref ct) = cached.content_type {
                    resp = resp.header(axum::http::header::CONTENT_TYPE, ct.clone());
                }
                resp = resp.header(
                    HeaderName::from_static("x-cache-idempotency"),
                    HeaderValue::from_static("HIT"),
                );
                return resp
                    .body(Body::from(cached.body.clone()))
                    .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response());
            }
        }
    }

    // 2. Execute inner handler
    let resp = next.run(req).await;

    // Only cache successful or definitive client/server responses (not transient 500s or in-progress)
    let status = resp.status();
    if status.is_success() || status == StatusCode::CREATED || status == StatusCode::NO_CONTENT {
        let (parts, body) = resp.into_parts();
        let content_type = parts.headers.get(axum::http::header::CONTENT_TYPE).cloned();

        // Convert body to bytes (bounded)
        if let Ok(bytes) = to_bytes(body, 2 * 1024 * 1024).await {
            if let Ok(mut guard) = IDEMPOTENCY_CACHE.write() {
                // Evict expired entries if cache is full
                if guard.len() >= MAX_CACHE_ENTRIES {
                    guard.retain(|_, v| v.created_at.elapsed() < CACHE_TTL);
                }
                guard.insert(
                    key,
                    CachedResponse {
                        status,
                        content_type,
                        body: bytes.clone(),
                        created_at: Instant::now(),
                    },
                );
            }
            return Response::from_parts(parts, Body::from(bytes));
        } else {
            return Response::from_parts(parts, Body::empty());
        }
    }

    resp
}
