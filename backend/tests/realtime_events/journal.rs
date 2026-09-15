use backend::events::{DomainEvent, EventJournal, ReplayOutcome};
use std::time::Duration;

use crate::support::TestDatabase;

#[tokio::test]
async fn durable_replay_preserves_sequence_order() {
    let database = TestDatabase::seeded("event-replay.db").await;
    let journal = EventJournal::init(database.pool.clone()).await.unwrap();

    for (path, action) in [
        ("/file1.txt", "create"),
        ("/file2.txt", "write"),
        ("/file3.txt", "delete"),
    ] {
        journal
            .append(DomainEvent::file_change("local", path, action), None)
            .await
            .unwrap();
    }

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
async fn replay_reports_expired_when_requested_sequence_predates_retention() {
    let database = TestDatabase::seeded("event-expiry.db").await;
    let journal = EventJournal::init(database.pool.clone()).await.unwrap();

    for index in 0..120 {
        journal
            .append(
                DomainEvent::file_change("local", format!("/file_{index}.txt"), "create"),
                None,
            )
            .await
            .unwrap();
    }

    sqlx::query("DELETE FROM event_journal WHERE sequence <= 20")
        .execute(&database.pool)
        .await
        .unwrap();

    match journal
        .get_since(Some(journal.epoch()), 1, 200)
        .await
        .unwrap()
    {
        ReplayOutcome::Expired { latest_sequence } => assert_eq!(latest_sequence, 120),
        other => panic!("expected expired replay, got {other:?}"),
    }

    match journal
        .get_since(Some(journal.epoch()), 110, 200)
        .await
        .unwrap()
    {
        ReplayOutcome::Events(events) => {
            assert_eq!(events.len(), 10);
            assert_eq!(events.first().unwrap().sequence, 111);
            assert_eq!(events.last().unwrap().sequence, 120);
        }
        other => panic!("expected retained replay events, got {other:?}"),
    }
}

#[tokio::test]
async fn durable_consumer_cursor_protects_unprocessed_rows_from_vacuum() {
    let database = TestDatabase::seeded("event-consumer-cursor.db").await;
    let journal = EventJournal::init(database.pool.clone()).await.unwrap();

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

    assert_eq!(journal.consumer_cursor("events-test").await.unwrap(), 0);
    assert_eq!(journal.vacuum(Duration::ZERO).await.unwrap(), 0);

    journal
        .store_consumer_cursor("events-test", first_id)
        .await
        .unwrap();
    assert_eq!(journal.vacuum(Duration::ZERO).await.unwrap(), 1);

    let remaining: Vec<i64> = sqlx::query_scalar("SELECT id FROM event_journal ORDER BY id")
        .fetch_all(&database.pool)
        .await
        .unwrap();
    assert_eq!(remaining, vec![second_id]);
}
