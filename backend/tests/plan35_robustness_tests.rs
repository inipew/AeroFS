use backend::config::AppConfig;
use backend::db::init_db;
use backend::errors::AppError;
use backend::state::{AppState, SearchState};
use backend::transfer::{OperationPlan, RetryPolicy};
use axum::extract::FromRef;
use std::time::Duration;
use tempfile::TempDir;

#[test]
fn test_plan35_retry_policy_behavior() {
    let policy = RetryPolicy::default();
    assert!(policy.should_retry(0));
    assert!(policy.should_retry(1));
    assert!(policy.should_retry(2));
    assert!(!policy.should_retry(3));
    assert_eq!(policy.backoff(0), Duration::from_millis(500));
    assert_eq!(policy.backoff(1), Duration::from_millis(1000));
    assert_eq!(policy.backoff(2), Duration::from_millis(2000));
}

#[test]
fn test_plan35_operation_plan_and_execution_result() {
    let plan = OperationPlan::default();
    assert_eq!(plan.retry.max_retries, 3);
}

#[test]
fn test_plan35_structured_error_responses() {
    let error = AppError::BadRequest("bad request".to_string());
    assert!(error.to_string().contains("bad request"));
}

#[tokio::test]
async fn test_plan35_backpressure_semaphores() {
    let temp = TempDir::new().unwrap();
    let db_path = temp.path().join("test.db");
    let storage_dir = temp.path().join("storage");
    std::fs::create_dir_all(&storage_dir).unwrap();

    let mut config = AppConfig::default();
    config.database.url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    config.filesystem.default_local_root = storage_dir;

    let db = init_db(&config.database.url).await.unwrap();
    let state = AppState::new_with_db(config, db).await;

    // Archive is still a legacy AppState-owned capability until H5.
    assert_eq!(state.archive_semaphore.available_permits(), 4);
    {
        let permit1 = state.archive_semaphore.acquire().await.unwrap();
        let permit2 = state.archive_semaphore.acquire().await.unwrap();
        assert_eq!(state.archive_semaphore.available_permits(), 2);
        drop(permit1);
        drop(permit2);
    }
    assert_eq!(state.archive_semaphore.available_permits(), 4);

    // Search limiter moved behind SearchState/SearchService in H1+H2. Verify configured
    // capacity through the capability instead of reaching into the raw semaphore.
    let search = SearchState::from_ref(&state);
    assert_eq!(search.service.available_capacity(), 8);
}
