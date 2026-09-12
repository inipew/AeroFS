use axum::{
    body::{to_bytes, Body, Bytes, HttpBody},
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

impl CacheEntry {
    fn created_at(&self) -> Instant {
        match self {
            Self::InProgress(created_at) => *created_at,
            Self::Completed(response) => response.created_at,
        }
    }
}

static IDEMPOTENCY_CACHE: LazyLock<Arc<RwLock<HashMap<String, CacheEntry>>>> =
    LazyLock::new(|| Arc::new(RwLock::new(HashMap::new())));

const IDEMPOTENCY_HEADER: &str = "idempotency-key";
const CACHE_TTL: Duration = Duration::from_secs(300); // 5 minutes
const IN_FLIGHT_TIMEOUT: Duration = Duration::from_secs(30); // 30 seconds
const MAX_CACHE_ENTRIES: usize = 1000;
const MAX_CACHED_RESPONSE_BYTES: usize = 2 * 1024 * 1024;

/// Axum middleware for transparent scoped Idempotency-Key deduplication on mutating requests.
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

    // Include path + query so the same key cannot collide across semantically
    // different request targets.
    let scoped_key = format!("{}:{}:{}:{}", auth_scope, method, req.uri(), raw_key);

    {
        if let Ok(mut guard) = IDEMPOTENCY_CACHE.write() {
            if guard.len() >= MAX_CACHE_ENTRIES {
                guard.retain(|_, value| match value {
                    CacheEntry::InProgress(t) => t.elapsed() < IN_FLIGHT_TIMEOUT,
                    CacheEntry::Completed(c) => c.created_at.elapsed() < CACHE_TTL,
                });
            }
            if guard.len() >= MAX_CACHE_ENTRIES && !guard.contains_key(&scoped_key) {
                if let Some(oldest_key) = guard
                    .iter()
                    .min_by_key(|(_, entry)| entry.created_at())
                    .map(|(key, _)| key.clone())
                {
                    guard.remove(&oldest_key);
                }
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
                        return crate::errors::AppError::ConcurrentIdempotentRequest(
                            "An identical request is currently being processed".to_string(),
                        )
                        .into_response();
                    }
                    _ => {}
                }
            }

            guard.insert(scoped_key.clone(), CacheEntry::InProgress(Instant::now()));
        }
    }

    let resp = next.run(req).await;
    let status = resp.status();
    if status.is_success() || status == StatusCode::CREATED || status == StatusCode::NO_CONTENT {
        // Never consume a body unless the framework can prove it fits the cache.
        // Some small JSON responses do not carry Content-Length, but Axum's body
        // still exposes an exact/finite size hint. Unknown/streaming bodies are
        // passed through untouched so idempotency can never truncate a success.
        let header_len = resp
            .headers()
            .get(axum::http::header::CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<usize>().ok());
        let hinted_upper = resp
            .body()
            .size_hint()
            .upper()
            .and_then(|value| usize::try_from(value).ok());
        let cacheable_len = header_len
            .or(hinted_upper)
            .filter(|length| *length <= MAX_CACHED_RESPONSE_BYTES);

        if cacheable_len.is_none() {
            if let Ok(mut guard) = IDEMPOTENCY_CACHE.write() {
                guard.remove(&scoped_key);
            }
            return resp;
        }

        let (parts, body) = resp.into_parts();
        let content_type = parts.headers.get(axum::http::header::CONTENT_TYPE).cloned();
        match to_bytes(body, MAX_CACHED_RESPONSE_BYTES).await {
            Ok(bytes) => {
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
                Response::from_parts(parts, Body::from(bytes))
            }
            Err(error) => {
                // A body whose declared/hinted upper bound fits the cache should
                // not cross the limit. Treat this as a server contract failure,
                // never as a false successful response with an empty body.
                if let Ok(mut guard) = IDEMPOTENCY_CACHE.write() {
                    guard.remove(&scoped_key);
                }
                tracing::error!(?error, "failed to buffer cacheable idempotent response");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    } else {
        if let Ok(mut guard) = IDEMPOTENCY_CACHE.write() {
            guard.remove(&scoped_key);
        }
        resp
    }
}
