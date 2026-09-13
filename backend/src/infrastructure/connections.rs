use crate::db::DbPool;
use crate::domain::{Connection, ConnectionStatus, ProviderKind};
use crate::errors::AppError;
use crate::infrastructure::CredentialStore;
use crate::ports::connections::{
    ConnectionRepository, ConnectionSecretError, SecretMutation,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::path::PathBuf;
use std::sync::Arc;

type ConnectionDbRow = (
    String,
    String,
    String,
    Option<String>,
    Option<i64>,
    Option<String>,
    String,
    i64,
    i64,
    String,
    String,
);

#[derive(Clone)]
pub struct SqliteConnectionRepository {
    db: DbPool,
    credentials: Arc<CredentialStore>,
}

impl SqliteConnectionRepository {
    pub fn new(db: DbPool, credentials: Arc<CredentialStore>) -> Self {
        Self { db, credentials }
    }

    fn provider_kind(provider: &str) -> ProviderKind {
        match provider {
            "ftp" => ProviderKind::Ftp,
            "ftps" => ProviderKind::Ftps,
            "sftp" => ProviderKind::Sftp,
            "s3" => ProviderKind::S3,
            _ => ProviderKind::Local,
        }
    }

    fn provider_name(provider: ProviderKind) -> &'static str {
        match provider {
            ProviderKind::Ftp => "ftp",
            ProviderKind::Ftps => "ftps",
            ProviderKind::Sftp => "sftp",
            ProviderKind::S3 => "s3",
            ProviderKind::Local => "local",
        }
    }

    fn row_to_connection(row: ConnectionDbRow) -> Connection {
        let (
            id,
            name,
            provider,
            host,
            port,
            username,
            base_path,
            read_only,
            enabled,
            created_at,
            updated_at,
        ) = row;
        Connection {
            id,
            name,
            provider: Self::provider_kind(&provider),
            host,
            port: port.map(|value| value as u16),
            username,
            base_path,
            read_only: read_only != 0,
            enabled: enabled != 0,
            status: if enabled != 0 {
                ConnectionStatus::Connected
            } else {
                ConnectionStatus::Disconnected
            },
            error_message: None,
            created_at: DateTime::parse_from_rfc3339(&created_at)
                .map(|value| value.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&updated_at)
                .map(|value| value.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        }
    }

    async fn save_secret(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        id: &str,
        secret: &str,
        now: &str,
    ) -> Result<(), AppError> {
        let encrypted = self.credentials.encrypt(secret)?;
        sqlx::query("INSERT INTO connection_credentials (connection_id, credential_type, encrypted_secret, created_at) VALUES (?, 'password_or_key', ?, ?) ON CONFLICT(connection_id) DO UPDATE SET encrypted_secret = excluded.encrypted_secret, created_at = excluded.created_at")
            .bind(id)
            .bind(encrypted)
            .bind(now)
            .execute(&mut **tx)
            .await
            .map_err(|error| anyhow::anyhow!("Failed to save credential: {}", error))?;
        Ok(())
    }
}

#[async_trait]
impl ConnectionRepository for SqliteConnectionRepository {
    async fn local_root_override(&self) -> Result<Option<PathBuf>, AppError> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT value FROM system_settings WHERE key = 'local_root'",
        )
        .fetch_optional(&self.db)
        .await
        .map_err(|error| anyhow::anyhow!("Failed to load local_root setting: {}", error))?;
        Ok(row.map(|value| PathBuf::from(value.0)))
    }

    async fn load_enabled(&self) -> Result<Vec<Connection>, AppError> {
        let rows: Vec<ConnectionDbRow> = sqlx::query_as(
            "SELECT id, name, provider, host, port, username, base_path, read_only, enabled, created_at, updated_at FROM connections WHERE enabled = 1 ORDER BY name ASC",
        )
        .fetch_all(&self.db)
        .await
        .map_err(|error| anyhow::anyhow!("Failed to load enabled storage connections: {}", error))?;
        Ok(rows.into_iter().map(Self::row_to_connection).collect())
    }

    async fn load_secret(&self, id: &str) -> Result<Option<String>, ConnectionSecretError> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT encrypted_secret FROM connection_credentials WHERE connection_id = ?",
        )
        .bind(id)
        .fetch_optional(&self.db)
        .await
        .map_err(|error| ConnectionSecretError::Persistence(error.to_string()))?;
        match row {
            Some((encrypted,)) => self
                .credentials
                .decrypt(&encrypted)
                .map(Some)
                .map_err(|error| ConnectionSecretError::Decryption(error.to_string())),
            None => Ok(None),
        }
    }

    async fn can_read(&self, user_id: &str, id: &str) -> Result<bool, AppError> {
        let allowed: Option<(i64,)> = sqlx::query_as(
            "SELECT can_read FROM permissions WHERE user_id = ? AND connection_id = ? LIMIT 1",
        )
        .bind(user_id)
        .bind(id)
        .fetch_optional(&self.db)
        .await
        .map_err(|error| anyhow::anyhow!("Database error: {}", error))?;
        Ok(allowed.map(|row| row.0 != 0).unwrap_or(false))
    }

    async fn list(&self, user_id: Option<&str>, is_admin: bool) -> Result<Vec<Connection>, AppError> {
        let rows: Vec<ConnectionDbRow> = if is_admin {
            sqlx::query_as(
                "SELECT id, name, provider, host, port, username, base_path, read_only, enabled, created_at, updated_at FROM connections ORDER BY name ASC",
            )
            .fetch_all(&self.db)
            .await
            .map_err(|error| anyhow::anyhow!("Database error: {}", error))?
        } else {
            let user_id = user_id.ok_or_else(|| AppError::Forbidden("Missing user identity".into()))?;
            sqlx::query_as(
                "SELECT c.id, c.name, c.provider, c.host, c.port, c.username, c.base_path, c.read_only, c.enabled, c.created_at, c.updated_at FROM connections c JOIN permissions p ON p.connection_id = c.id WHERE p.user_id = ? AND p.can_read = 1 ORDER BY c.name ASC",
            )
            .bind(user_id)
            .fetch_all(&self.db)
            .await
            .map_err(|error| anyhow::anyhow!("Database error: {}", error))?
        };
        Ok(rows.into_iter().map(Self::row_to_connection).collect())
    }

    async fn get(&self, id: &str) -> Result<Option<Connection>, AppError> {
        let row: Option<ConnectionDbRow> = sqlx::query_as(
            "SELECT id, name, provider, host, port, username, base_path, read_only, enabled, created_at, updated_at FROM connections WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.db)
        .await
        .map_err(|error| anyhow::anyhow!("Database error: {}", error))?;
        Ok(row.map(Self::row_to_connection))
    }

    async fn exists(&self, id: &str) -> Result<bool, AppError> {
        let row: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM connections WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.db)
            .await
            .map_err(|error| anyhow::anyhow!("Failed to verify connection before deletion: {}", error))?;
        Ok(row.is_some())
    }

    async fn create(&self, connection: &Connection, secret: Option<&str>) -> Result<(), AppError> {
        let mut tx = self
            .db
            .begin()
            .await
            .map_err(|error| anyhow::anyhow!("Failed to begin transaction: {}", error))?;
        let created_at = connection.created_at.to_rfc3339();
        let updated_at = connection.updated_at.to_rfc3339();
        sqlx::query(
            "INSERT INTO connections (id, name, provider, host, port, username, base_path, read_only, enabled, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&connection.id)
        .bind(&connection.name)
        .bind(Self::provider_name(connection.provider))
        .bind(&connection.host)
        .bind(connection.port.map(|value| value as i64))
        .bind(&connection.username)
        .bind(&connection.base_path)
        .bind(if connection.read_only { 1 } else { 0 })
        .bind(if connection.enabled { 1 } else { 0 })
        .bind(&created_at)
        .bind(&updated_at)
        .execute(&mut *tx)
        .await
        .map_err(|error| anyhow::anyhow!("Failed to save connection: {}", error))?;
        if let Some(secret) = secret.filter(|value| !value.trim().is_empty()) {
            self.save_secret(&mut tx, &connection.id, secret, &updated_at)
                .await?;
        }
        tx.commit()
            .await
            .map_err(|error| anyhow::anyhow!("Failed to commit connection transaction: {}", error))?;
        Ok(())
    }

    async fn update(
        &self,
        connection: &Connection,
        secret: SecretMutation,
    ) -> Result<(), AppError> {
        let mut tx = self
            .db
            .begin()
            .await
            .map_err(|error| anyhow::anyhow!("Failed to begin transaction: {}", error))?;
        let updated_at = connection.updated_at.to_rfc3339();
        let result = sqlx::query("UPDATE connections SET name = ?, host = ?, port = ?, username = ?, base_path = ?, read_only = ?, enabled = ?, updated_at = ? WHERE id = ?")
            .bind(&connection.name)
            .bind(&connection.host)
            .bind(connection.port.map(|value| value as i64))
            .bind(&connection.username)
            .bind(&connection.base_path)
            .bind(if connection.read_only { 1 } else { 0 })
            .bind(if connection.enabled { 1 } else { 0 })
            .bind(&updated_at)
            .bind(&connection.id)
            .execute(&mut *tx)
            .await
            .map_err(|error| anyhow::anyhow!("Failed to update connection in DB: {}", error))?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!(
                "Connection '{}' not found",
                connection.id
            )));
        }
        match secret {
            SecretMutation::Keep => {}
            SecretMutation::Replace(value) => {
                self.save_secret(&mut tx, &connection.id, &value, &updated_at)
                    .await?;
            }
            SecretMutation::Clear => {
                sqlx::query("DELETE FROM connection_credentials WHERE connection_id = ?")
                    .bind(&connection.id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|error| anyhow::anyhow!("Failed to clear credential: {}", error))?;
            }
        }
        tx.commit()
            .await
            .map_err(|error| anyhow::anyhow!("Failed to commit update transaction: {}", error))?;
        Ok(())
    }

    async fn disable(&self, id: &str) -> Result<(), AppError> {
        let result = sqlx::query("UPDATE connections SET enabled = 0, updated_at = ? WHERE id = ?")
            .bind(Utc::now().to_rfc3339())
            .bind(id)
            .execute(&self.db)
            .await
            .map_err(|error| anyhow::anyhow!("Failed to disable connection: {}", error))?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("Connection '{}' not found", id)));
        }
        Ok(())
    }

    async fn delete_with_transfer_barrier(&self, id: &str) -> Result<bool, AppError> {
        let mut tx = self
            .db
            .begin()
            .await
            .map_err(|error| anyhow::anyhow!("Failed to begin connection deletion transaction: {}", error))?;
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE transfer_jobs SET status = CASE WHEN status = 'queued' THEN 'cancelled' ELSE 'cancellation_requested' END, updated_at = ? WHERE (source_connection_id = ? OR destination_connection_id = ?) AND status IN ('queued', 'running', 'cancellation_requested')",
        )
        .bind(&now)
        .bind(id)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|error| anyhow::anyhow!("Failed to persist transfer cancellation barrier: {}", error))?;
        sqlx::query("DELETE FROM connection_credentials WHERE connection_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|error| anyhow::anyhow!("Failed to delete connection credential: {}", error))?;
        let result = sqlx::query("DELETE FROM connections WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|error| anyhow::anyhow!("Failed to delete connection: {}", error))?;
        if result.rows_affected() == 0 {
            return Ok(false);
        }
        tx.commit()
            .await
            .map_err(|error| anyhow::anyhow!("Failed to commit connection deletion: {}", error))?;
        Ok(true)
    }
}
