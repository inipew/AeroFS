//! EventBus abstraction — decouples publishers from subscribers (§36-37, §121).
//! FileService previously did `state.transfer_manager.broadcast_event(WsEvent::file_change(..))`
//! which leaked transfer subsystem into file domain. Now: `event_bus.publish(AppEvent::File(..))`.

use super::{DomainEvent, EventEnvelope, EventJournal};
use std::sync::Arc;

#[async_trait::async_trait]
pub trait EventBus: Send + Sync {
    async fn publish(
        &self,
        event: DomainEvent,
        aggregate_id: Option<&str>,
    ) -> anyhow::Result<EventEnvelope>;
    fn subscribe(&self) -> tokio::sync::broadcast::Receiver<EventEnvelope>;
    fn epoch(&self) -> String;
    fn latest_sequence(&self) -> u64;
}

#[async_trait::async_trait]
pub trait EventStore: Send + Sync {
    async fn append(
        &self,
        event: DomainEvent,
        aggregate_id: Option<&str>,
    ) -> anyhow::Result<EventEnvelope>;
    async fn replay_since(
        &self,
        client_epoch: Option<&str>,
        last_sequence: u64,
        limit: usize,
    ) -> anyhow::Result<super::ReplayOutcome>;
}

#[async_trait::async_trait]
impl EventBus for EventJournal {
    async fn publish(
        &self,
        event: DomainEvent,
        aggregate_id: Option<&str>,
    ) -> anyhow::Result<EventEnvelope> {
        self.append(event, aggregate_id).await
    }
    fn subscribe(&self) -> tokio::sync::broadcast::Receiver<EventEnvelope> {
        EventJournal::subscribe(self)
    }
    fn epoch(&self) -> String {
        EventJournal::epoch(self).to_string()
    }
    fn latest_sequence(&self) -> u64 {
        EventJournal::latest_sequence(self)
    }
}

#[async_trait::async_trait]
impl EventStore for EventJournal {
    async fn append(
        &self,
        event: DomainEvent,
        aggregate_id: Option<&str>,
    ) -> anyhow::Result<EventEnvelope> {
        EventJournal::append(self, event, aggregate_id).await
    }
    async fn replay_since(
        &self,
        client_epoch: Option<&str>,
        last_sequence: u64,
        limit: usize,
    ) -> anyhow::Result<super::ReplayOutcome> {
        self.get_since(client_epoch, last_sequence, limit).await
    }
}

/// Helper to publish typed AppEvent via any EventBus impl
pub async fn publish_app_event(
    bus: &Arc<EventJournal>,
    app_event: super::app_event::AppEvent,
    aggregate_id: Option<&str>,
) -> anyhow::Result<EventEnvelope> {
    let domain: DomainEvent = app_event.into();
    bus.append(domain, aggregate_id).await
}
