use crate::db::DbPool;
use crate::events::{DomainEvent, EventEnvelope, EventJournal, ReplayOutcome};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct RealtimePrincipal {
    pub user_id: String,
    pub is_admin: bool,
}

impl RealtimePrincipal {
    pub fn new(user_id: impl Into<String>, is_admin: bool) -> Self {
        Self {
            user_id: user_id.into(),
            is_admin,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealtimeEpochInfo {
    pub epoch: String,
    pub latest_sequence: u64,
}

/// Narrow capability used by the WebSocket transport.
///
/// The HTTP adapter receives this service through `RealtimeState` and therefore
/// cannot reach the application database, event journal, or runtime container
/// directly. Concrete dependencies remain composed once in `bootstrap`.
#[derive(Clone)]
pub struct RealtimeService {
    db: DbPool,
    journal: Arc<EventJournal>,
    shutdown_token: CancellationToken,
}

impl RealtimeService {
    pub fn new(
        db: DbPool,
        journal: Arc<EventJournal>,
        shutdown_token: CancellationToken,
    ) -> Self {
        Self {
            db,
            journal,
            shutdown_token,
        }
    }

    pub fn shutdown_token(&self) -> CancellationToken {
        self.shutdown_token.clone()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EventEnvelope> {
        self.journal.subscribe()
    }

    pub fn epoch_info(&self) -> RealtimeEpochInfo {
        RealtimeEpochInfo {
            epoch: self.journal.epoch().to_string(),
            latest_sequence: self.journal.latest_sequence(),
        }
    }

    pub async fn replay(
        &self,
        client_epoch: Option<&str>,
        last_sequence: u64,
        limit: usize,
    ) -> anyhow::Result<ReplayOutcome> {
        self.journal
            .get_since(client_epoch, last_sequence, limit)
            .await
    }

    pub async fn authorized_connections(&self, principal: &RealtimePrincipal) -> HashSet<String> {
        let mut connections = HashSet::new();
        connections.insert("local".to_string());

        if principal.is_admin {
            return connections;
        }

        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT connection_id FROM permissions WHERE user_id = ? AND can_read = 1",
        )
        .bind(&principal.user_id)
        .fetch_all(&self.db)
        .await
        .unwrap_or_default();

        for (connection_id,) in rows {
            connections.insert(connection_id);
        }

        connections
    }

    pub fn is_event_authorized(
        &self,
        event: &DomainEvent,
        principal: &RealtimePrincipal,
        authorized_connections: &HashSet<String>,
    ) -> bool {
        event_authorized(event, principal, authorized_connections)
    }
}

fn event_authorized(
    event: &DomainEvent,
    principal: &RealtimePrincipal,
    authorized_connections: &HashSet<String>,
) -> bool {
    if principal.is_admin {
        return true;
    }

    match event {
        DomainEvent::TransferProgress(value)
        | DomainEvent::TransferCompleted(value)
        | DomainEvent::TransferFailed(value)
        | DomainEvent::TransferCancelled(value) => {
            if let Some(owner) = value.get("user_id").and_then(|value| value.as_str()) {
                if owner == principal.user_id {
                    return true;
                }
            }

            let source = value
                .get("source_connection_id")
                .and_then(|value| value.as_str());
            let destination = value
                .get("destination_connection_id")
                .and_then(|value| value.as_str());

            if value.get("transfer_type").and_then(|value| value.as_str()) == Some("upload") {
                return destination
                    .is_some_and(|connection_id| authorized_connections.contains(connection_id));
            }

            if value.get("transfer_type").and_then(|value| value.as_str()) == Some("download") {
                return source
                    .is_some_and(|connection_id| authorized_connections.contains(connection_id));
            }

            match (source, destination) {
                (Some(source), Some(destination)) => {
                    authorized_connections.contains(source)
                        && authorized_connections.contains(destination)
                }
                (Some(source), None) => authorized_connections.contains(source),
                (None, Some(destination)) => authorized_connections.contains(destination),
                _ => false,
            }
        }
        DomainEvent::FileChange { connection_id, .. } => {
            authorized_connections.contains(connection_id)
        }
        DomainEvent::ResyncRequired { .. } => true,
        DomainEvent::PermissionChanged { .. } => true,
        DomainEvent::FullSync { .. } => true,
    }
}

#[cfg(test)]
mod tests {
    use super::{event_authorized, RealtimePrincipal, RealtimeService};
    use crate::events::DomainEvent;
    use std::collections::HashSet;

    #[test]
    fn upload_progress_is_authorized_by_destination_only() {
        let principal = RealtimePrincipal::new("user1", false);
        let allowed = HashSet::from(["destination".to_string()]);
        let event = DomainEvent::TransferProgress(serde_json::json!({
            "transfer_type": "upload",
            "source_connection_id": "upload",
            "destination_connection_id": "destination"
        }));
        assert!(event_authorized(&event, &principal, &allowed));
    }

    #[test]
    fn transfer_owner_is_always_authorized() {
        let allowed = HashSet::new();
        let event = DomainEvent::TransferProgress(serde_json::json!({
            "user_id": "user1",
            "transfer_type": "copy",
            "source_connection_id": "source",
            "destination_connection_id": "destination"
        }));
        assert!(event_authorized(
            &event,
            &RealtimePrincipal::new("user1", false),
            &allowed
        ));
        assert!(!event_authorized(
            &event,
            &RealtimePrincipal::new("user2", false),
            &allowed
        ));
    }

    #[test]
    fn non_upload_transfer_requires_both_endpoints() {
        let principal = RealtimePrincipal::new("user1", false);
        let allowed = HashSet::from(["destination".to_string()]);
        let event = DomainEvent::TransferProgress(serde_json::json!({
            "transfer_type": "copy",
            "source_connection_id": "source",
            "destination_connection_id": "destination"
        }));
        assert!(!event_authorized(&event, &principal, &allowed));
    }

    #[test]
    fn admin_can_observe_any_event() {
        let event = DomainEvent::FileChange {
            connection_id: "private".to_string(),
            path: "/secret".to_string(),
            action: "write".to_string(),
            old_path: None,
            parent_path: Some("/".to_string()),
            old_parent_path: None,
        };
        assert!(event_authorized(
            &event,
            &RealtimePrincipal::new("admin", true),
            &HashSet::new()
        ));
    }

    #[test]
    fn realtime_service_type_remains_cloneable() {
        fn assert_clone<T: Clone>() {}
        assert_clone::<RealtimeService>();
    }
}
