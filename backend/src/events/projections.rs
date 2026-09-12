use crate::events::{DomainEvent, EventEnvelope, EventJournal};
use crate::runtime::TaskSupervisor;
use crate::services::MetadataCache;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

const CONSUMER_NAME: &str = "metadata-cache-invalidation";
const REPLAY_BATCH_SIZE: usize = 256;

/// Event-derived recovery projection for metadata cache invalidation.
///
/// Mutation use-cases still invalidate synchronously for immediate consistency.
/// This subscriber provides convergence after lag/restart and keeps cache recovery
/// independent from API/application call paths.
pub struct MetadataCacheEventSubscriber;

impl MetadataCacheEventSubscriber {
    pub fn spawn(
        supervisor: &TaskSupervisor,
        event_journal: Arc<EventJournal>,
        cache: Arc<MetadataCache>,
        shutdown: CancellationToken,
    ) {
        let mut events = event_journal.subscribe();

        supervisor.spawn("metadata_cache_event_subscriber", async move {
            let mut cursor = match event_journal.consumer_cursor(CONSUMER_NAME).await {
                Ok(cursor) => cursor,
                Err(error) => {
                    tracing::error!(%error, "failed to load metadata cache event cursor");
                    0
                }
            };

            if let Err(error) = replay_backlog(&event_journal, &cache, &mut cursor).await {
                tracing::error!(%error, "failed to replay metadata cache event backlog");
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
                                    &cache,
                                    &mut cursor,
                                    envelope,
                                )
                                .await
                                {
                                    tracing::warn!(%error, "metadata cache event handling failed; replaying durable journal");
                                    if let Err(replay_error) = replay_backlog(
                                        &event_journal,
                                        &cache,
                                        &mut cursor,
                                    )
                                    .await
                                    {
                                        tracing::error!(%replay_error, "metadata cache backlog replay failed");
                                    }
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                                tracing::warn!(skipped, cursor, "metadata cache event subscriber lagged; replaying durable journal");
                                if let Err(error) = replay_backlog(
                                    &event_journal,
                                    &cache,
                                    &mut cursor,
                                )
                                .await
                                {
                                    tracing::error!(%error, "metadata cache backlog replay failed after lag");
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
    cache: &MetadataCache,
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
            process_envelope(event_journal, cache, cursor, envelope).await?;
        }

        if batch_len < REPLAY_BATCH_SIZE {
            return Ok(());
        }
    }
}

async fn process_envelope(
    event_journal: &EventJournal,
    cache: &MetadataCache,
    cursor: &mut i64,
    envelope: EventEnvelope,
) -> anyhow::Result<()> {
    let journal_id = envelope.journal_id;

    if let DomainEvent::FileChange {
        connection_id,
        path,
        old_path,
        parent_path,
        old_parent_path,
        ..
    } = envelope.event
    {
        cache.invalidate_prefix(&connection_id, &path).await;
        if let Some(old_path) = old_path {
            cache.invalidate_prefix(&connection_id, &old_path).await;
        }
        if let Some(parent_path) = parent_path {
            cache.invalidate(&connection_id, &parent_path).await;
        }
        if let Some(old_parent_path) = old_parent_path {
            cache.invalidate(&connection_id, &old_parent_path).await;
        }
    }

    if let Some(journal_id) = journal_id {
        event_journal
            .store_consumer_cursor(CONSUMER_NAME, journal_id)
            .await?;
        *cursor = (*cursor).max(journal_id);
    }

    Ok(())
}
