use backend::cli::daemon_lock::DaemonLock;
use backend::config::AppConfig;
use backend::create_router;
use backend::db::init_db;
use backend::AppState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;
use tokio::net::TcpListener;

async fn setup_test_context() -> (AppState, tempfile::TempDir) {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("lifecycle_test.db");
    let storage_dir = temp.path().join("storage");
    std::fs::create_dir_all(&storage_dir).unwrap();

    let mut config = AppConfig::default();
    config.database.url = format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap());
    config.filesystem.default_local_root = storage_dir;

    let db = init_db(&config.database.url).await.unwrap();
    let state = AppState::new_with_db(config, db).await;

    (state, temp)
}

#[tokio::test]
async fn test_app_runtime_task_tracker_and_cancellation_drain() {
    let (state, _temp) = setup_test_context().await;

    let task_executed = Arc::new(AtomicBool::new(false));
    let task_cleaned_up = Arc::new(AtomicBool::new(false));

    let executed_clone = task_executed.clone();
    let cleaned_clone = task_cleaned_up.clone();
    let token = state.runtime.shutdown_token.clone();

    // Spawn long-running background worker inside state.runtime.task_tracker
    state.runtime.task_tracker.spawn(async move {
        executed_clone.store(true, Ordering::SeqCst);
        tokio::select! {
            _ = token.cancelled() => {
                // Perform graceful worker cleanup
                tokio::time::sleep(Duration::from_millis(50)).await;
                cleaned_clone.store(true, Ordering::SeqCst);
            }
            _ = tokio::time::sleep(Duration::from_secs(60)) => {}
        }
    });

    // Wait a brief moment to ensure task has started
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(
        task_executed.load(Ordering::SeqCst),
        "Task should be running"
    );
    assert!(
        !task_cleaned_up.load(Ordering::SeqCst),
        "Task should not be cleaned up yet"
    );

    // Trigger shutdown cancellation
    state.runtime.shutdown_token.cancel();
    state.runtime.task_tracker.close();

    // Wait for all tracked tasks to complete within deadline
    let drain_result =
        tokio::time::timeout(Duration::from_secs(2), state.runtime.task_tracker.wait()).await;
    assert!(
        drain_result.is_ok(),
        "Task tracker drain should complete within deadline"
    );
    assert!(
        task_cleaned_up.load(Ordering::SeqCst),
        "Task should have cleanly executed its shutdown branch"
    );
}

#[tokio::test]
async fn test_graceful_shutdown_idle_server_terminates_promptly() {
    let (state, _temp) = setup_test_context().await;
    let app = create_router(state.clone());

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local_addr = listener.local_addr().unwrap();

    let shutdown_token = state.runtime.shutdown_token.clone();
    let token_for_shutdown = shutdown_token.clone();

    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                token_for_shutdown.cancelled().await;
            })
            .await
    });

    // Verify server is listening
    let stream = tokio::net::TcpStream::connect(local_addr).await;
    assert!(
        stream.is_ok(),
        "Server should accept incoming TCP connections"
    );
    drop(stream);

    // Trigger shutdown
    let start = std::time::Instant::now();
    shutdown_token.cancel();

    let server_result = tokio::time::timeout(Duration::from_secs(2), server_handle).await;
    assert!(
        server_result.is_ok(),
        "Server should terminate cleanly under 2 seconds"
    );
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_secs(1),
        "Idle server shutdown should finish in < 1s, took {:?}",
        elapsed
    );
}

#[tokio::test]
async fn test_daemon_lock_lifecycle_acquire_and_release() {
    let temp = tempdir().unwrap();
    let lock_file = temp.path().join("aerofs.lock");

    // 1. First acquisition succeeds
    let lock1 = DaemonLock::acquire(&lock_file);
    assert!(lock1.is_ok(), "First lock acquisition should succeed");
    let lock1 = lock1.unwrap();

    // 2. Second acquisition must fail while first is held
    let lock2 = DaemonLock::acquire(&lock_file);
    assert!(
        lock2.is_err(),
        "Second lock acquisition should fail while first is held"
    );

    // 3. Release first lock
    lock1.release();

    // 4. Now second acquisition must succeed
    let lock3 = DaemonLock::acquire(&lock_file);
    assert!(
        lock3.is_ok(),
        "Lock acquisition should succeed after release"
    );
    let lock3 = lock3.unwrap();
    lock3.release();
}
