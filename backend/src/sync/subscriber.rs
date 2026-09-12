use crate::events::{DomainEvent, EventEnvelope, EventJournal};
use crate::runtime::TaskSupervisor;
use crate::sync::SyncManager;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

const CONSUMER_NAME: &str = "sync-transfer-completion";
const REPLAY_BATCH_SIZE: usize = 256;

/// Canonical durable bridge from transfer domain events into Sync state.
///
/// The subscriber maintains a SQLite-backed cursor over `event_journal.id`, so
/// lifecycle events are replayed after lag or process restart. Live delivery is
/// still used for low latency, but durable replay is the recovery source of truth.
pub struct SyncEventSubscriber;

impl SyncEventSubscriber {
    pub fn spawn(
        supervisor: &TaskSupervisor,
        event_journal: Arc<EventJournal>,
        sync_manager: Arc<SyncManager>,
        shutdown: CancellationToken,
    ) {
        // Subscribe before replaying so events appended during startup recovery are
        // buffered by broadcast and can be de-duplicated by the durable cursor.
        let mut events = event_journal.subscribe();

        supervisor.spawn("sync_event_subscriber", async move {
            let mut cursor = match event_journal.consumer_cursor(CONSUMER_NAME).await {
                Ok(cursor) => cursor,
                Err(error) => {
                    tracing::error!(%error, "failed to load sync event cursor");
                    0
                }
            };

            if let Err(error) = replay_backlog(
                &event_journal,
                &sync_manager,
                &mut cursor,
            )
            .await
            {
                tracing::error!(%error, "failed to replay sync event backlog");
            }

            loop {
                tokio::select! {
                    _ = shutdown.cancelled() => break,
                    result = events.recv() => {
                        match result {
                            Ok(envelope) => {
                                if let Some(journal_id) = envelope.journal_id {
                                    if journal_id <= cursor {
                                        continue;
                                    }
                                }

                                if let Err(error) = process_envelope(
                                    &event_journal,
                                    &sync_manager,
                                    &mut cursor,
                                    envelope,
                                )
                                .await
                                {
                                    tracing::warn!(%error, "sync event handling failed; replaying from durable cursor");
                                    if let Err(replay_error) = replay_backlog(
                                        &event_journal,
                                        &sync_manager,
                                        &mut cursor,
                                    )
                                    .await
                                    {
                                        tracing::error!(%replay_error, "sync event backlog replay failed");
                                    }
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                                tracing::warn!(skipped, cursor, "sync event subscriber lagged; replaying durable journal");
                                if let Err(error) = replay_backlog(
                                    &event_journal,
                                    &sync_manager,
                                    &mut cursor,
                                )
                                .await
                                {
                                    tracing::error!(%error, "sync event backlog replay failed after lag");
                                }
                            }
                            Err(broadcast::error::RecvError::Closed) => break,
                        }
                    }
                }
            }
        });
    }
}

async fn replay_backlog(
    event_journal: &EventJournal,
    sync_manager: &SyncManager,
    cursor: &mut i64,
) -> anyhow::Result<()> {
    loop {
        let batch = event_journal
            .durable_events_after(*cursor, REPLAY_BATCH_SIZE)
            .await?;
        if batch.is_empty() {
            return Ok(());
        }

        let batch_len = batch.len();
        for envelope in batch {
            process_envelope(event_journal, sync_manager, cursor, envelope).await?;
        }

        if batch_len < REPLAY_BATCH_SIZE {
            return Ok(());
        }
    }
}

async fn process_envelope(
    event_journal: &EventJournal,
    sync_manager: &SyncManager,
    cursor: &mut i64,
    envelope: EventEnvelope,
) -> anyhow::Result<()> {
    let journal_id = envelope.journal_id;

    let completion = match envelope.event {
        DomainEvent::TransferCompleted(ref value) => value
            .get("id")
            .and_then(|v| v.as_str())
            .map(|id| (id.to_owned(), true)),
        DomainEvent::TransferFailed(ref value) | DomainEvent::TransferCancelled(ref value) => value
            .get("id")
            .and_then(|v| v.as_str())
            .map(|id| (id.to_owned(), false)),
        _ => None,
    };

    if let Some((job_id, success)) = completion {
        sync_manager
            .notify_transfer_completed(&job_id, success)
            .await?;
    }

    // Only durable events advance the consumer cursor. Transient progress events
    // are intentionally ignored by this projection.
    if let Some(journal_id) = journal_id {
        event_journal
            .store_consumer_cursor(CONSUMER_NAME, journal_id)
            .await?;
        *cursor = (*cursor).max(journal_id);
    }

    Ok(())
}
