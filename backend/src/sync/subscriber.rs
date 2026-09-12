use crate::events::{DomainEvent, EventJournal};
use crate::runtime::TaskSupervisor;
use crate::sync::SyncManager;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

/// Canonical bridge from durable transfer domain events into Sync state.
/// Sync intentionally does not subscribe to TransferManager's legacy completion
/// broadcast; EventJournal is the single completion source of truth.
pub struct SyncEventSubscriber;

impl SyncEventSubscriber {
    pub fn spawn(
        supervisor: &TaskSupervisor,
        event_journal: Arc<EventJournal>,
        sync_manager: Arc<SyncManager>,
        shutdown: CancellationToken,
    ) {
        let mut events = event_journal.subscribe();
        supervisor.spawn("sync_event_subscriber", async move {
            loop {
                tokio::select! {
                    _ = shutdown.cancelled() => break,
                    result = events.recv() => {
                        match result {
                            Ok(envelope) => {
                                let completion = match envelope.event {
                                    DomainEvent::TransferCompleted(ref value) => value
                                        .get("id")
                                        .and_then(|v| v.as_str())
                                        .map(|id| (id.to_owned(), true)),
                                    DomainEvent::TransferFailed(ref value)
                                    | DomainEvent::TransferCancelled(ref value) => value
                                        .get("id")
                                        .and_then(|v| v.as_str())
                                        .map(|id| (id.to_owned(), false)),
                                    _ => None,
                                };

                                if let Some((job_id, success)) = completion {
                                    if let Err(error) = sync_manager
                                        .notify_transfer_completed(&job_id, success)
                                        .await
                                    {
                                        tracing::warn!(
                                            job_id = %job_id,
                                            %error,
                                            "sync transfer completion handling failed"
                                        );
                                    }
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                                tracing::warn!(skipped, "sync event subscriber lagged");
                            }
                            Err(broadcast::error::RecvError::Closed) => break,
                        }
                    }
                }
            }
        });
    }
}
