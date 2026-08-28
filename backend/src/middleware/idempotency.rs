use axum::{
    body::{to_bytes, Body, Bytes},
    extract::Request,
    http::{header::HeaderName, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
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

#[derive(Clone)]
enum CacheEntry {
    InProgress(Instant),
    Completed(CachedResponse),
}

static IDEMPOTENCY_CACHE: LazyLock<Arc<RwLock<HashMap<String, CacheEntry>>>> =
    LazyLock::new(|| Arc::new(RwLock::new(HashMap::new())));

const IDEMPOTENCY_HEADER: &str = "idempotency-key";
const CACHE_TTL: Duration = Duration::from_secs(300); // 5 minutes
const IN_FLIGHT_TIMEOUT: Duration = Duration::from_secs(30); // 30 seconds
const MAX_CACHE_ENTRIES: usize = 1000;

/// Axum middleware for transparent scoped Idempotency-Key deduplication on mutating requests
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

    let raw_key = match key_opt {
        Some(k) => k,
        None => return next.run(req).await,
    };

    // Construct composite isolation key: auth_scope + method + path + idempotency_key
    let auth_scope = req
        .headers()
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|c| {
            c.split(';')
                .find(|s| s.trim().starts_with("session_id="))
                .map(|s| s.trim().to_string())
        })
        .or_else(|| {
            req.headers()
                .get(axum::http::header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "anon".to_string());

    let scoped_key = format!("{}:{}:{}:{}", auth_scope, method, req.uri().path(), raw_key);

    // 1. Check existing cache & atomic reservation
    {
        if let Ok(mut guard) = IDEMPOTENCY_CACHE.write() {
            // Evict expired entries if cache is full
            if guard.len() >= MAX_CACHE_ENTRIES {
                guard.retain(|_, v| match v {
                    CacheEntry::InProgress(t) => t.elapsed() < IN_FLIGHT_TIMEOUT,
                    CacheEntry::Completed(c) => c.created_at.elapsed() < CACHE_TTL,
                });
            }

            if let Some(entry) = guard.get(&scoped_key) {
                match entry {
                    CacheEntry::Completed(cached) if cached.created_at.elapsed() < CACHE_TTL => {
                        tracing::debug!("Idempotency hit for scoped key: {}", scoped_key);
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
                    CacheEntry::InProgress(t) if t.elapsed() < IN_FLIGHT_TIMEOUT => {
                        // Concurrent request with same idempotency key is currently processing
                        return Response::builder()
                            .status(StatusCode::CONFLICT)
                            .body(Body::from(
                                r#"{"error":{"code":"CONCURRENT_IDEMPOTENT_REQUEST","message":"An identical request is currently being processed"}}"#,
                            ))
                            .unwrap_or_else(|_| StatusCode::CONFLICT.into_response());
                    }
                    _ => {}
                }
            }

            // Reserve in-flight execution
            guard.insert(scoped_key.clone(), CacheEntry::InProgress(Instant::now()));
        }
    }

    // 2. Execute inner handler
    let resp = next.run(req).await;

    // 3. Cache completed response
    let status = resp.status();
    if status.is_success() || status == StatusCode::CREATED || status == StatusCode::NO_CONTENT {
        let (parts, body) = resp.into_parts();
        let content_type = parts.headers.get(axum::http::header::CONTENT_TYPE).cloned();

        // Convert body to bytes (bounded 2 MB limit)
        if let Ok(bytes) = to_bytes(body, 2 * 1024 * 1024).await {
            if let Ok(mut guard) = IDEMPOTENCY_CACHE.write() {
                guard.insert(
                    scoped_key,
                    CacheEntry::Completed(CachedResponse {
                        status,
                        content_type,
                        body: bytes.clone(),
                        created_at: Instant::now(),
                    }),
                );
            }
            return Response::from_parts(parts, Body::from(bytes));
        } else {
            if let Ok(mut guard) = IDEMPOTENCY_CACHE.write() {
                guard.remove(&scoped_key);
            }
            return Response::from_parts(parts, Body::empty());
        }
    } else {
        // Clear reservation on failure so client can retry with same key
        if let Ok(mut guard) = IDEMPOTENCY_CACHE.write() {
            guard.remove(&scoped_key);
        }
    }

    resp
}
