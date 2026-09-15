use axum::response::IntoResponse;
use backend::{
    errors::{AppError, VfsError},
    domain::RetryPolicy,
};

#[test]
fn app_errors_map_to_stable_http_status_classes() {
    assert_eq!(
        AppError::NotFound("file.txt".to_string())
            .into_response()
            .status(),
        axum::http::StatusCode::NOT_FOUND
    );
    assert_eq!(
        AppError::Conflict("version mismatch".to_string())
            .into_response()
            .status(),
        axum::http::StatusCode::CONFLICT
    );
    assert_eq!(
        AppError::ChecksumMismatch("hash invalid".to_string())
            .into_response()
            .status(),
        axum::http::StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[test]
fn retry_policy_distinguishes_terminal_from_transient_errors_and_backs_off_monotonically() {
    let policy = RetryPolicy::new(3);
    assert_eq!(policy.max_attempts, 3);

    for terminal in [
        AppError::NotFound("missing".to_string()),
        AppError::Forbidden("denied".to_string()),
        AppError::BadRequest("bad".to_string()),
    ] {
        assert!(!policy.is_retryable(&terminal));
    }

    for transient in [
        AppError::ChecksumMismatch("corrupted".to_string()),
        AppError::Vfs(VfsError::ConnectionError("drop".to_string())),
        AppError::Vfs(VfsError::IoError("reset".to_string())),
    ] {
        assert!(policy.is_retryable(&transient));
    }

    let backoffs = [
        policy.compute_backoff(1),
        policy.compute_backoff(2),
        policy.compute_backoff(3),
    ];
    assert!(backoffs.windows(2).all(|window| window[0] <= window[1]));
}
