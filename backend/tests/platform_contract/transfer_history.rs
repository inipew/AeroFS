use backend::infrastructure::transfer_history::SqliteTransferHistoryRepository;
use backend::services::TransferService;
use std::sync::Arc;

use crate::support::TestDatabase;

#[tokio::test]
async fn transfer_history_queries_repair_and_purge_operate_on_durable_rows() {
    let database = TestDatabase::seeded("platform_transfer_history.db").await;
    let transfers = TransferService::new(Arc::new(SqliteTransferHistoryRepository::new(
        database.pool.clone(),
    )));

    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO transfer_jobs (id, user_id, name, transfer_type, source_connection_id, source_path,
                destination_connection_id, destination_path, status, phase, transferred_bytes, total_bytes,
                speed_bytes_per_sec, checksum, created_at, updated_at)
         VALUES ('test_job_1', 'user_1', 'Upload Test', 'upload', 'local', '/file.txt', 'local', '/dst.txt',
                 'running', 'transferring', 500, 1000, 50, 'chk', ?, ?)",
    )
    .bind(&now)
    .bind(&now)
    .execute(&database.pool)
    .await
    .unwrap();

    let job = transfers.get_transfer("test_job_1").await.unwrap().unwrap();
    assert_eq!(job.name, "Upload Test");
    assert_eq!(job.total_bytes, 1000);
    assert_eq!(job.transferred_bytes, 500);

    let running = transfers
        .list_transfers_filtered(Some("running"), 10, None, None)
        .await
        .unwrap();
    assert_eq!(running.len(), 1);

    assert_eq!(transfers.repair_stuck_transfers(true).await.unwrap(), 1);
    assert_eq!(transfers.repair_stuck_transfers(false).await.unwrap(), 1);
    assert_eq!(
        transfers
            .get_transfer("test_job_1")
            .await
            .unwrap()
            .unwrap()
            .status
            .as_str(),
        "failed"
    );

    assert_eq!(transfers.purge_transfers_older_than(0, true).await.unwrap(), 1);
}
