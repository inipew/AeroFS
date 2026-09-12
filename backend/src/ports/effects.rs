use crate::domain::{Actor, ConnectionId};
use async_trait::async_trait;

#[async_trait]
pub trait FileMutationEffects: Send + Sync {
    async fn invalidate(&self, connection: &ConnectionId, path: &str);
    async fn invalidate_prefix(&self, connection: &ConnectionId, path: &str);
    async fn file_changed(
        &self,
        actor: &Actor,
        connection: &ConnectionId,
        path: &str,
        audit_action: &'static str,
        event_action: &'static str,
        details: Option<String>,
    );
    async fn file_renamed(
        &self,
        actor: &Actor,
        connection: &ConnectionId,
        from: &str,
        to: &str,
    );
}
