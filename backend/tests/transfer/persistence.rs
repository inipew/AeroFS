mod support;

use backend::transfer::TransferCheckpoint;
use chrono::Utc;
use support::TestDatabase;

#[tokio::test]
async fn transfer_checkpoint_round_trips_and_deletes_through_real_migrations() {
    let database = TestDatabase::migrated("transfer-checkpoint.db").await;
    let transfer_id = "checkpoint-round-trip";
    let checkpoint = TransferCheckpoint {
        transfer_id: transfer_id.to_string(),
        offset: 1_048_576,
        total: 5_242_880,
        staging_path: "/.file.txt.aerofs-part-1".to_string(),
        source_etag: Some("\"etag-abc-123\"".to_string()),
        source_version: Some("version-7".to_string()),
        checksum_so_far: Some("partial-checksum".to_string()),
        updated_at: Utc::now(),
    };

    checkpoint.save(&database.pool).await.unwrap();

    let loaded = TransferCheckpoint::load(&database.pool, transfer_id)
        .await
        .unwrap()
        .expect("checkpoint should be durable");
    assert_eq!(loaded.transfer_id, transfer_id);
    assert_eq!(loaded.offset, 1_048_576);
    assert_eq!(loaded.total, 5_242_880);
    assert_eq!(loaded.staging_path, "/.file.txt.aerofs-part-1");
    assert_eq!(loaded.source_etag.as_deref(), Some("\"etag-abc-123\""));
    assert_eq!(loaded.source_version.as_deref(), Some("version-7"));
    assert_eq!(loaded.checksum_so_far.as_deref(), Some("partial-checksum"));

    TransferCheckpoint::delete(&database.pool, transfer_id)
        .await
        .unwrap();
    assert!(
        TransferCheckpoint::load(&database.pool, transfer_id)
            .await
            .unwrap()
            .is_none()
    );
}
