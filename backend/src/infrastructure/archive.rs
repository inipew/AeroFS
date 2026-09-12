use crate::auth::audit::record_audit_log;
use crate::domain::{Actor, ConnectionId};
use crate::ports::archive::ArchiveEffects;
use crate::transfer::{TransferManager, WsEvent};
use async_trait::async_trait;

pub struct SqliteArchiveEffects {
    db: crate::db::DbPool,
    transfers: TransferManager,
}

impl SqliteArchiveEffects {
    pub fn new(db: crate::db::DbPool, transfers: TransferManager) -> Self {
        Self { db, transfers }
    }
}

#[async_trait]
impl ArchiveEffects for SqliteArchiveEffects {
    async fn archive_created(
        &self,
        actor: &Actor,
        connection: &ConnectionId,
        destination_path: &str,
    ) {
        record_audit_log(
            &self.db,
            Some(&actor.id),
            "ARCHIVE_COMPRESS",
            Some(connection.as_str()),
            Some(destination_path),
            "SUCCESS",
            None,
            Some(&format!("Created archive: {}", destination_path)),
        )
        .await;

        self.transfers
            .broadcast_event(WsEvent::file_change(
                connection.as_str(),
                destination_path,
                "create",
            ))
            .await;
    }

    async fn archive_extracted(
        &self,
        actor: &Actor,
        connection: &ConnectionId,
        archive_path: &str,
        destination_dir: &str,
        count: usize,
        skipped: usize,
        selected: bool,
    ) {
        let action = if selected {
            "ARCHIVE_EXTRACT_SELECTED"
        } else {
            "ARCHIVE_EXTRACT"
        };
        let details = if selected {
            format!("Extracted {} selected items to {}", count, destination_dir)
        } else {
            format!(
                "Extracted {} items (skipped {}) to {}",
                count, skipped, destination_dir
            )
        };

        record_audit_log(
            &self.db,
            Some(&actor.id),
            action,
            Some(connection.as_str()),
            Some(archive_path),
            "SUCCESS",
            None,
            Some(&details),
        )
        .await;

        self.transfers
            .broadcast_event(WsEvent::file_change(
                connection.as_str(),
                destination_dir,
                "extract",
            ))
            .await;
    }
}
