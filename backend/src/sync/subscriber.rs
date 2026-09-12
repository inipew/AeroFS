use crate::events::{DomainEvent, EventEnvelope, EventJournal};
use crate::runtime::{RestartPolicy, TaskCriticality, TaskSupervisor};
use crate::sync::SyncManager;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

const CONSUMER_NAME: &str = "sync-transfer-completion";
const TASK_NAME: &str = "sync_event_subscriber";
const REPLAY_BATCH_SIZE: usize = 256;
const RESTART_BACKOFF: Duration = Duration::from_secs(2);
const RESTART_BUDGET_RESET_AFTER: Duration = Duration::from_secs(5 * 60);
const MAX_RESTARTS_PER_BUDGET: u32 = 5;

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
        let health = supervisor.clone();
        let factory_shutdown = shutdown.clone();

        supervisor.spawn_resilient(
            TASK_NAME,
            TaskCriticality::ReadinessCritical,
            RestartPolicy::bounded(
                MAX_RESTARTS_PER_BUDGET,
                RESTART_BACKOFF,
                RESTART_BUDGET_RESET_AFTER,
            ),
            shutdown,
            move || {
                let event_journal = event_journal.clone();
                let sync_manager = sync_manager.clone();
                let shutdown = factory_shutdown.clone();
                let health = health.clone();
                async move {
                    run_subscriber(event_journal, sync_manager, shutdown, health).await
                }
            },
        );
    }
}

async fn run_subscriber(
    event_journal: Arc<EventJournal>,
    sync_manager: Arc<SyncManager>,
    shutdown: CancellationToken,
    health: TaskSupervisor,
) -> anyhow::Result<()> {
    // Every restart gets a fresh live subscription. Any events missed between worker
    // instances are recovered from the durable journal before live processing resumes.
    let mut events = event_journal.subscribe();
    let mut cursor = event_journal.consumer_cursor(CONSUMER_NAME).await?;

    replay_backlog(&event_journal, &sync_manager, &mut cursor).await?;
    health.record_success(TASK_NAME);

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => return Ok(()),
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
                            // Replay is the recovery source of truth. If recovery itself
                            // fails, leave this worker instance so the supervisor applies
                            // bounded backoff/restart instead of spinning in the live loop.
                            replay_backlog(
                                &event_journal,
                                &sync_manager,
                                &mut cursor,
                            )
                            .await?;
                        }
                        health.record_success(TASK_NAME);
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::warn!(skipped, cursor, "sync event subscriber lagged; replaying durable journal");
                        replay_backlog(
                            &event_journal,
                            &sync_manager,
                            &mut cursor,
                        )
                        .await?;
                        health.record_success(TASK_NAME);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        if shutdown.is_cancelled() {
                            return Ok(());
                        }
                        anyhow::bail!("sync event journal broadcast closed unexpectedly");
                    }
                }
            }
        }
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
