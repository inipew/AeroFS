use crate::domain::{Actor, ConnectionId};
use crate::errors::AppError;
use async_trait::async_trait;

#[async_trait]
pub trait FileAccessEffects: Send + Sync {
    async fn accessed(
        &self,
        actor: &Actor,
        connection: &ConnectionId,
        path: &str,
        audit_action: &'static str,
        details: Option<String>,
    );
}

#[async_trait]
pub trait FileMutationEffects: Send + Sync {
    async fn invalidate(&self, connection: &ConnectionId, path: &str);
    async fn invalidate_prefix(&self, connection: &ConnectionId, path: &str);

    /// Records durable side effects for an already-committed filesystem mutation.
    /// Failure must be surfaced because the provider mutation has already happened
    /// and projections may otherwise diverge silently.
    async fn file_changed(
        &self,
        actor: &Actor,
        connection: &ConnectionId,
        path: &str,
        audit_action: &'static str,
        event_action: &'static str,
        details: Option<String>,
    ) -> Result<(), AppError>;

    async fn file_renamed(
        &self,
        actor: &Actor,
        connection: &ConnectionId,
        from: &str,
        to: &str,
    ) -> Result<(), AppError>;

    async fn file_copied(
        &self,
        actor: &Actor,
        connection: &ConnectionId,
        from: &str,
        to: &str,
    ) -> Result<(), AppError>;
}
