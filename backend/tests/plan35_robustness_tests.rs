use backend::config::AppConfig;
use backend::db::init_db;
use backend::domain::operation::{
    FailureStrategy, OperationExecutionResult, OperationIntentType, OperationPlan, OperationStatus,
};
use backend::domain::policy::PermissionInheritanceMode;
use backend::domain::retry::RetryPolicy;
use backend::errors::{AppError, VfsError};
use backend::AppState;
use tempfile::tempdir;

#[test]
fn test_plan35_structured_error_responses() {
    let not_found = AppError::NotFound("file.txt".to_string());
    let resp = axum::response::IntoResponse::into_response(not_found);
    assert_eq!(resp.status(), axum::http::StatusCode::NOT_FOUND);

    let conflict = AppError::Conflict("version mismatch".to_string());
    let resp = axum::response::IntoResponse::into_response(conflict);
    assert_eq!(resp.status(), axum::http::StatusCode::CONFLICT);

    let checksum_err = AppError::ChecksumMismatch("hash invalid".to_string());
    let resp = axum::response::IntoResponse::into_response(checksum_err);
    assert_eq!(resp.status(), axum::http::StatusCode::UNPROCESSABLE_ENTITY);
}

#[test]
fn test_plan35_retry_policy_behavior() {
    let policy = RetryPolicy::new(3);

    assert_eq!(policy.max_attempts, 3);
    assert!(!policy.is_retryable(&AppError::NotFound("missing".to_string())));
    assert!(!policy.is_retryable(&AppError::Forbidden("denied".to_string())));
    assert!(!policy.is_retryable(&AppError::BadRequest("bad".to_string())));
    assert!(policy.is_retryable(&AppError::ChecksumMismatch("corrupted".to_string())));
    assert!(
        policy.is_retryable(&AppError::Vfs(VfsError::ConnectionError(
            "drop".to_string()
        )))
    );
    assert!(policy.is_retryable(&AppError::Vfs(VfsError::IoError("reset".to_string()))));

    let b1 = policy.compute_backoff(1);
    let b2 = policy.compute_backoff(2);
    let b3 = policy.compute_backoff(3);
    assert!(b1 <= b2, "Backoff should be monotonically non-decreasing");
    assert!(b2 <= b3, "Backoff should be monotonically non-decreasing");
}

#[test]
fn test_plan35_operation_plan_and_execution_result() {
    let plan = OperationPlan {
        id: "plan-123".to_string(),
        intent_type: OperationIntentType::Copy,
        source_connection_id: "local".to_string(),
        source_paths: vec![],
        destination_connection_id: Some("local".to_string()),
        destination_path: None,
        failure_strategy: FailureStrategy::ContinueOnFailure,
        permission_mode: PermissionInheritanceMode::InheritParent,
        overwrite_mode: None,
    };

    assert_eq!(plan.failure_strategy, FailureStrategy::ContinueOnFailure);

    let mut result = OperationExecutionResult::new(plan.id.clone(), 3);
    assert_eq!(result.status, OperationStatus::Executing);

    result.succeeded_items.push("file1.txt".to_string());
    result
        .failed_items
        .push(("file2.txt".to_string(), "Permission denied".to_string()));
    result.succeeded_items.push("file3.txt".to_string());

    assert!(!result.is_success());
    result.finalize();
    assert_eq!(result.status, OperationStatus::Partial);
}

#[tokio::test]
async fn test_plan35_backpressure_semaphores() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("sem_test.db");
    let storage_dir = temp.path().join("storage");
    std::fs::create_dir_all(&storage_dir).unwrap();

    let mut config = AppConfig::default();
    config.database.url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    config.filesystem.default_local_root = storage_dir;

    let db = init_db(&config.database.url).await.unwrap();
    let state = AppState::new_with_db(config, db).await;

    // 1. Check archive semaphore permits
    assert_eq!(state.archive_semaphore.available_permits(), 4);
    {
        let permit1 = state.archive_semaphore.acquire().await.unwrap();
        let permit2 = state.archive_semaphore.acquire().await.unwrap();
        assert_eq!(state.archive_semaphore.available_permits(), 2);
        drop(permit1);
        drop(permit2);
    }
    assert_eq!(state.archive_semaphore.available_permits(), 4);

    // 2. Check search semaphore permits
    assert_eq!(state.search_semaphore.available_permits(), 8);
    {
        let permit = state.search_semaphore.acquire().await.unwrap();
        assert_eq!(state.search_semaphore.available_permits(), 7);
        drop(permit);
    }
    assert_eq!(state.search_semaphore.available_permits(), 8);
}
