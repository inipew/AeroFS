use crate::auth::{check_permission, PermissionAction, UserInfo};
use crate::config::AppConfig;
use crate::domain::{Actor, ConnectionId};
use crate::errors::{AppError, VfsError};
use crate::events::EventJournal;
use crate::ports::{
    authorization::{Authorization, FileAction},
    effects::FileMutationEffects,
    filesystem::FileSystemResolver,
    settings::FileSettings,
};
use crate::services::cache::MetadataCache;
use crate::vfs::{registry::ProviderRegistry, FileSystem};
use async_trait::async_trait;
use std::sync::Arc;

pub struct SqliteAuthorization {
    db: crate::db::DbPool,
}

impl SqliteAuthorization {
    pub fn new(db: crate::db::DbPool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Authorization for SqliteAuthorization {
    async fn authorize(
        &self,
        actor: &Actor,
        connection: &ConnectionId,
        action: FileAction,
    ) -> Result<(), AppError> {
        let user = UserInfo {
            id: actor.id.clone(),
            username: actor.username.clone(),
            is_admin: actor.is_admin,
        };
        let permission = match action {
            FileAction::List | FileAction::Read => PermissionAction::Read,
            FileAction::Download => PermissionAction::Download,
            FileAction::Create => PermissionAction::Create,
            FileAction::Write => PermissionAction::Write,
            FileAction::Delete => PermissionAction::Delete,
        };
        check_permission(&self.db, &user, connection.as_str(), permission).await
    }
}

pub struct RegistryFileSystemResolver {
    registry: Arc<ProviderRegistry>,
}

impl RegistryFileSystemResolver {
    pub fn new(registry: Arc<ProviderRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl FileSystemResolver for RegistryFileSystemResolver {
    async fn resolve(&self, connection: &ConnectionId) -> Result<Arc<dyn FileSystem>, VfsError> {
        self.registry.get(connection.as_str()).await.ok_or_else(|| {
            VfsError::ConnectionError(format!("Connection '{}' not found", connection.as_str()))
        })
    }
}

pub struct SqliteFileSettings {
    db: crate::db::DbPool,
    config: Arc<AppConfig>,
}

impl SqliteFileSettings {
    pub fn new(db: crate::db::DbPool, config: Arc<AppConfig>) -> Self {
        Self { db, config }
    }
}

#[async_trait]
impl FileSettings for SqliteFileSettings {
    async fn show_hidden_default(&self) -> Result<bool, AppError> {
        let configured: Option<String> = sqlx::query_scalar(
            "SELECT value FROM system_settings WHERE key = 'show_hidden_default'",
        )
        .fetch_optional(&self.db)
        .await
        .unwrap_or(None);
        Ok(configured
            .map(|value| value == "true")
            .unwrap_or(self.config.filesystem.show_hidden_default))
    }

    async fn max_editable_size(&self) -> Result<u64, AppError> {
        let configured: Option<String> = sqlx::query_scalar(
            "SELECT value FROM system_settings WHERE key = 'max_editable_size'",
        )
        .fetch_optional(&self.db)
        .await
        .unwrap_or(None);
        Ok(configured
            .and_then(|value| value.parse().ok())
            .unwrap_or(self.config.limits.max_editable_size))
    }

    fn max_directory_entries(&self) -> usize {
        self.config.limits.max_directory_entries
    }
}

pub struct SqliteFileMutationEffects {
    db: crate::db::DbPool,
    cache: Arc<MetadataCache>,
    event_journal: Arc<EventJournal>,
}

impl SqliteFileMutationEffects {
    pub fn new(
        db: crate::db::DbPool,
        cache: Arc<MetadataCache>,
        event_journal: Arc<EventJournal>,
    ) -> Self {
        Self {
            db,
            cache,
            event_journal,
        }
    }
}

#[async_trait]
impl FileMutationEffects for SqliteFileMutationEffects {
    async fn invalidate(&self, connection: &ConnectionId, path: &str) {
        self.cache.invalidate(connection.as_str(), path).await;
    }

    async fn invalidate_prefix(&self, connection: &ConnectionId, path: &str) {
        self.cache.invalidate_prefix(connection.as_str(), path).await;
    }

    async fn file_changed(
        &self,
        actor: &Actor,
        connection: &ConnectionId,
        path: &str,
        audit_action: &'static str,
        event_action: &'static str,
        details: Option<String>,
    ) {
        crate::auth::audit::record_audit_log(
            &self.db,
            Some(&actor.id),
            audit_action,
            Some(connection.as_str()),
            Some(path),
            "SUCCESS",
            None,
            details.as_deref(),
        )
        .await;
        let _ = self
            .event_journal
            .append(
                crate::events::DomainEvent::file_change(
                    connection.as_str(),
                    path,
                    event_action,
                ),
                None,
            )
            .await;
    }

    async fn file_renamed(
        &self,
        actor: &Actor,
        connection: &ConnectionId,
        from: &str,
        to: &str,
    ) {
        crate::auth::audit::record_audit_log(
            &self.db,
            Some(&actor.id),
            "FILE_RENAME",
            Some(connection.as_str()),
            Some(from),
            "SUCCESS",
            None,
            Some(&format!("Renamed to: {}", to)),
        )
        .await;
        let _ = self
            .event_journal
            .append(
                crate::events::DomainEvent::file_rename(connection.as_str(), from, to),
                None,
            )
            .await;
    }
}
