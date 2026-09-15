use backend::events::{EventJournal, ReplayOutcome};

use crate::support::TestDatabase;

#[tokio::test]
async fn replay_with_stale_epoch_requires_full_resynchronization() {
    let database = TestDatabase::seeded("event-epoch.db").await;
    let journal = EventJournal::init(database.pool.clone()).await.unwrap();
    let current_epoch = journal.epoch().to_string();

    match journal
        .get_since(Some("stale-client-epoch"), 50, 10)
        .await
        .unwrap()
    {
        ReplayOutcome::EpochMismatch {
            current_epoch: reported_epoch,
            ..
        } => assert_eq!(reported_epoch, current_epoch),
        other => panic!("expected epoch mismatch, got {other:?}"),
    }
}
