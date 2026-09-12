use axum::{
    body::{to_bytes, Body, Bytes, HttpBody},
    extract::Request,
    http::{header::HeaderName, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use sha2::{Digest, Sha256};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RequestFingerprint([u8; 32]);

#[derive(Clone)]
enum CacheEntry {
    InProgress {
        created_at: Instant,
        fingerprint: RequestFingerprint,
    },
    Completed {
        response: CachedResponse,
        fingerprint: RequestFingerprint,
    },
}

impl CacheEntry {
    fn created_at(&self) -> Instant {
        match self {
            Self::InProgress { created_at, .. } => *created_at,
            Self::Completed { response, .. } => response.created_at,
        }
    }

    fn fingerprint(&self) -> RequestFingerprint {
        match self {
            Self::InProgress { fingerprint, .. } | Self::Completed { fingerprint, .. } => {
                *fingerprint
            }
        }
    }
}

#[derive(Clone)]
enum CacheLookup {
    Hit(CachedResponse),
    InProgress,
    PayloadConflict,
    Stale,
}

static IDEMPOTENCY_CACHE: LazyLock<Arc<RwLock<HashMap<String, CacheEntry>>>> =
    LazyLock::new(|| Arc::new(RwLock::new(HashMap::new())));

const IDEMPOTENCY_HEADER: &str = "idempotency-key";
const CACHE_TTL: Duration = Duration::from_secs(300); // 5 minutes
const IN_FLIGHT_TIMEOUT: Duration = Duration::from_secs(30); // 30 seconds
const MAX_CACHE_ENTRIES: usize = 1000;
const MAX_IDEMPOTENT_REQUEST_BYTES: usize = 2 * 1024 * 1024;
const MAX_CACHED_RESPONSE_BYTES: usize = 2 * 1024 * 1024;

fn request_fingerprint(body: &[u8]) -> RequestFingerprint {
    let digest = Sha256::digest(body);
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&digest);
    RequestFingerprint(bytes)
}

fn scope_fingerprint(scope: &str) -> String {
    hex::encode(Sha256::digest(scope.as_bytes()))
}

fn classify_cache_entry(
    entry: &CacheEntry,
    fingerprint: RequestFingerprint,
    now: Instant,
) -> CacheLookup {
    let age = now.saturating_duration_since(entry.created_at());
    let is_fresh = match entry {
        CacheEntry::InProgress { .. } => age < IN_FLIGHT_TIMEOUT,
        CacheEntry::Completed { .. } => age < CACHE_TTL,
    };

    if !is_fresh {
        return CacheLookup::Stale;
    }

    if entry.fingerprint() != fingerprint {
        return CacheLookup::PayloadConflict;
    }

    match entry {
        CacheEntry::Completed { response, .. } => CacheLookup::Hit(response.clone()),
        CacheEntry::InProgress { .. } => CacheLookup::InProgress,
    }
}

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

    if let Some(content_length) = req
        .headers()
        .get(axum::http::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok())
    {
        if content_length > MAX_IDEMPOTENT_REQUEST_BYTES {
            return crate::errors::AppError::PayloadTooLarge(format!(
                "Idempotency-Key request bodies are limited to {} bytes; use an upload session or omit the idempotency key for larger streaming bodies",
                MAX_IDEMPOTENT_REQUEST_BYTES
            ))
            .into_response();
        }
    }

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

    let uri = req.uri().clone();
    let (parts, body) = req.into_parts();
    let request_body = match to_bytes(body, MAX_IDEMPOTENT_REQUEST_BYTES).await {
        Ok(bytes) => bytes,
        Err(error) => {
            tracing::warn!(
                ?error,
                max_bytes = MAX_IDEMPOTENT_REQUEST_BYTES,
                "failed to buffer idempotent request body"
            );
            return crate::errors::AppError::PayloadTooLarge(format!(
                "Idempotency-Key request body could not be buffered within the {} byte limit",
                MAX_IDEMPOTENT_REQUEST_BYTES
            ))
            .into_response();
        }
    };
    let fingerprint = request_fingerprint(&request_body);
    let req = Request::from_parts(parts, Body::from(request_body));

    // Scope by authenticated principal, method, full URI (including query), and
    // the caller-provided key. The payload fingerprint is stored in the entry
    // rather than appended to the key: reusing one idempotency key for a different
    // payload is a conflict, not a second independent operation.
    // Hash the auth scope before composing/logging the cache key so bearer/session
    // credentials are never retained as plaintext cache keys or debug output.
    let scoped_key = format!(
        "{}:{}:{}:{}",
        scope_fingerprint(&auth_scope),
        method,
        uri,
        raw_key
    );

    {
        if let Ok(mut guard) = IDEMPOTENCY_CACHE.write() {
            if guard.len() >= MAX_CACHE_ENTRIES {
                guard.retain(|_, value| match value {
                    CacheEntry::InProgress { created_at, .. } => {
                        created_at.elapsed() < IN_FLIGHT_TIMEOUT
                    }
                    CacheEntry::Completed { response, .. } => response.created_at.elapsed() < CACHE_TTL,
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
                match classify_cache_entry(entry, fingerprint, Instant::now()) {
                    CacheLookup::Hit(cached) => {
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
                    CacheLookup::InProgress => {
                        return crate::errors::AppError::ConcurrentIdempotentRequest(
                            "An identical request is currently being processed".to_string(),
                        )
                        .into_response();
                    }
                    CacheLookup::PayloadConflict => {
                        return crate::errors::AppError::Conflict(
                            "Idempotency-Key was already used for the same endpoint with a different request payload"
                                .to_string(),
                        )
                        .into_response();
                    }
                    CacheLookup::Stale => {}
                }
            }

            guard.insert(
                scoped_key.clone(),
                CacheEntry::InProgress {
                    created_at: Instant::now(),
                    fingerprint,
                },
            );
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
                        CacheEntry::Completed {
                            response: CachedResponse {
                                status,
                                content_type,
                                body: bytes.clone(),
                                created_at: Instant::now(),
                            },
                            fingerprint,
                        },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_fingerprint_changes_when_payload_changes() {
        let first = request_fingerprint(br#"{"path":"/one"}"#);
        let second = request_fingerprint(br#"{"path":"/two"}"#);
        assert_ne!(first, second);
        assert_eq!(first, request_fingerprint(br#"{"path":"/one"}"#));
    }

    #[test]
    fn fresh_entry_with_different_payload_is_conflict() {
        let entry = CacheEntry::InProgress {
            created_at: Instant::now(),
            fingerprint: request_fingerprint(b"one"),
        };

        assert!(matches!(
            classify_cache_entry(&entry, request_fingerprint(b"two"), Instant::now()),
            CacheLookup::PayloadConflict
        ));
    }

    #[test]
    fn fresh_entry_with_same_payload_preserves_inflight_semantics() {
        let fingerprint = request_fingerprint(b"same");
        let entry = CacheEntry::InProgress {
            created_at: Instant::now(),
            fingerprint,
        };

        assert!(matches!(
            classify_cache_entry(&entry, fingerprint, Instant::now()),
            CacheLookup::InProgress
        ));
    }

    #[test]
    fn stale_entry_can_be_reused_with_new_payload() {
        let entry = CacheEntry::InProgress {
            created_at: Instant::now() - IN_FLIGHT_TIMEOUT - Duration::from_secs(1),
            fingerprint: request_fingerprint(b"old"),
        };

        assert!(matches!(
            classify_cache_entry(&entry, request_fingerprint(b"new"), Instant::now()),
            CacheLookup::Stale
        ));
    }
}
