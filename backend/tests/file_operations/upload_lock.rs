use backend::{errors::AppError, services::UploadLockManager};

#[tokio::test]
async fn upload_mutation_lock_is_scoped_by_connection_and_destination_path() {
    let manager = UploadLockManager::new();
    let held = manager
        .try_acquire("conn-s3", "/movies/big.mkv")
        .await
        .unwrap();

    let duplicate = manager.try_acquire("conn-s3", "/movies/big.mkv").await;
    assert!(matches!(duplicate, Err(AppError::Conflict(message)) if message.contains("already in progress")));

    let _other_connection = manager
        .try_acquire("conn-ftp", "/movies/big.mkv")
        .await
        .expect("same path on another connection must have an independent mutation lock");
    let _other_path = manager
        .try_acquire("conn-s3", "/movies/other.mkv")
        .await
        .expect("another path on the same connection must have an independent mutation lock");

    drop(held);
    assert!(manager
        .try_acquire("conn-s3", "/movies/big.mkv")
        .await
        .is_ok(), "dropping the guard must release the path synchronously");
}
