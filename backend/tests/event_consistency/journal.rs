use backend::events::{DomainEvent, EventJournal, ReplayOutcome};
use std::time::Duration;

use crate::support::TestDatabase;

#[tokio::test]
async fn durable_replay_is_ordered_and_resumes_after_the_last_seen_sequence() {
    let database = TestDatabase::seeded("event-replay.db").await;
    let journal = EventJournal::init(database.pool.clone())
        .await
        .expect("initialize event journal");

    journal
        .append(DomainEvent::file_change("local", "/file1.txt", "create"), None)
        .await
        .unwrap();
    journal
        .append(DomainEvent::file_change("local", "/file2.txt", "write"), None)
        .await
        .unwrap();
    journal
        .append(DomainEvent::file_change("local", "/file3.txt", "delete"), None)
        .await
        .unwrap();

    let missed = match journal
        .get_since(Some(journal.epoch()), 1, 100)
        .await
        .unwrap()
    {
        ReplayOutcome::Events(events) => events,
        other => panic!("expected replay events, got {other:?}"),
    };
    assert_eq!(missed.len(), 2);
    assert_eq!(missed[0].sequence, 2);
    assert_eq!(missed[1].sequence, 3);

    let all = match journal
        .get_since(Some(journal.epoch()), 0, 100)
        .await
        .unwrap()
    {
        ReplayOutcome::Events(events) => events,
        other => panic!("expected replay events, got {other:?}"),
    };
    assert_eq!(
        all.iter().map(|event| event.sequence).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
}

#[tokio::test]
async fn durable_consumer_cursor_fences_vacuum_until_rows_are_processed() {
    let database = TestDatabase::seeded("event-vacuum.db").await;
    let journal = EventJournal::init(database.pool.clone())
        .await
        .expect("initialize event journal");

    let first = journal
        .append(DomainEvent::file_change("local", "/one.txt", "write"), None)
        .await
        .unwrap();
    let second = journal
        .append(DomainEvent::file_change("local", "/two.txt", "write"), None)
        .await
        .unwrap();
    let first_id = first.journal_id.expect("first event must be durable");
    let second_id = second.journal_id.expect("second event must be durable");

    sqlx::query("UPDATE event_journal SET created_at = '2000-01-01T00:00:00Z'")
        .execute(&database.pool)
        .await
        .unwrap();

    assert_eq!(journal.consumer_cursor("event-consistency").await.unwrap(), 0);
    assert_eq!(journal.vacuum(Duration::ZERO).await.unwrap(), 0);

    journal
        .store_consumer_cursor("event-consistency", first_id)
        .await
        .unwrap();
    assert_eq!(journal.vacuum(Duration::ZERO).await.unwrap(), 1);

    let remaining: Vec<i64> = sqlx::query_scalar("SELECT id FROM event_journal ORDER BY id")
        .fetch_all(&database.pool)
        .await
        .unwrap();
    assert_eq!(remaining, vec![second_id]);
}
