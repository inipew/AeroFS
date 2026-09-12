use crate::auth::{check_permission, PermissionAction, UserInfo};
use crate::config::AppConfig;
use crate::domain::{Actor, ConnectionId};
use crate::errors::{AppError, VfsError};
use crate::ports::{authorization::{Authorization, FileAction}, filesystem::FileSystemResolver, settings::FileSettings};
use crate::vfs::{registry::ProviderRegistry, FileSystem};
use async_trait::async_trait;
use std::sync::Arc;

pub struct SqliteAuthorization { db: crate::db::DbPool }
impl SqliteAuthorization { pub fn new(db: crate::db::DbPool) -> Self { Self { db } } }
#[async_trait]
impl Authorization for SqliteAuthorization {
    async fn authorize(&self, actor: &Actor, connection: &ConnectionId, action: FileAction) -> Result<(), AppError> {
        let user = UserInfo { id: actor.id.clone(), username: actor.username.clone(), is_admin: actor.is_admin };
        let permission = match action { FileAction::List => PermissionAction::Read };
        check_permission(&self.db, &user, connection.as_str(), permission).await
    }
}

pub struct RegistryFileSystemResolver { registry: Arc<ProviderRegistry> }
impl RegistryFileSystemResolver { pub fn new(registry: Arc<ProviderRegistry>) -> Self { Self { registry } } }
#[async_trait]
impl FileSystemResolver for RegistryFileSystemResolver {
    async fn resolve(&self, connection: &ConnectionId) -> Result<Arc<dyn FileSystem>, VfsError> {
        self.registry.get(connection.as_str()).await.ok_or_else(|| VfsError::ConnectionError(format!("Connection '{}' not found", connection.as_str())))
    }
}

pub struct SqliteFileSettings { db: crate::db::DbPool, config: Arc<AppConfig> }
impl SqliteFileSettings { pub fn new(db: crate::db::DbPool, config: Arc<AppConfig>) -> Self { Self { db, config } } }
#[async_trait]
impl FileSettings for SqliteFileSettings {
    async fn show_hidden_default(&self) -> Result<bool, AppError> {
        let configured: Option<String> = sqlx::query_scalar("SELECT value FROM system_settings WHERE key = 'show_hidden_default'").fetch_optional(&self.db).await.unwrap_or(None);
        Ok(configured.map(|value| value == "true").unwrap_or(self.config.filesystem.show_hidden_default))
    }
    fn max_directory_entries(&self) -> usize { self.config.limits.max_directory_entries }
}
