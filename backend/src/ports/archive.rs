use crate::domain::{Actor, ConnectionId};
use async_trait::async_trait;

#[async_trait]
pub trait ArchiveEffects: Send + Sync {
    async fn archive_created(
        &self,
        actor: &Actor,
        connection: &ConnectionId,
        destination_path: &str,
    );

    async fn archive_extracted(
        &self,
        actor: &Actor,
        connection: &ConnectionId,
        archive_path: &str,
        destination_dir: &str,
        count: usize,
        skipped: usize,
        selected: bool,
    );
}
