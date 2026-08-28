use crate::auth::credentials::{decrypt_secret, derive_master_key};
use crate::config::AppConfig;
use crate::db::DbPool;
use crate::domain::SftpAuth;
use crate::transfer::TransferManager;
use crate::vfs::opendal::{
    build_fs_operator, build_ftp_operator, build_s3_operator, build_sftp_operator,
    OpenDalFileSystem,
};
use crate::vfs::FileSystem;
use chrono::Utc;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub db: DbPool,
    pub providers: Arc<RwLock<HashMap<String, Arc<dyn FileSystem>>>>,
    pub connection_errors: Arc<RwLock<HashMap<String, String>>>,
    pub transfer_manager: TransferManager,
}

impl AppState {
    pub async fn new_with_db(config: AppConfig, db: DbPool) -> Self {
        let master_key = derive_master_key(&config.security.session_secret);
        let providers_map = Arc::new(RwLock::new(HashMap::new()));
        let connection_errors = Arc::new(RwLock::new(HashMap::new()));
        let transfer_manager = TransferManager::new(
            Arc::clone(&providers_map),
            db.clone(),
            config.limits.max_concurrent_transfers,
        );

        let state = Self {
            config: Arc::new(config),
            db,
            providers: providers_map,
            connection_errors,
            transfer_manager,
        };

        // Initialize and register all connections from DB
        state.load_providers_from_db(&master_key).await;

        state
    }

    pub async fn get_provider(&self, connection_id: &str) -> Option<Arc<dyn FileSystem>> {
        let providers = self.providers.read().await;
        providers.get(connection_id).cloned()
    }

    pub async fn register_provider(&self, connection_id: String, provider: Arc<dyn FileSystem>) {
        let mut providers = self.providers.write().await;
        providers.insert(connection_id.clone(), provider);
        self.clear_connection_error(&connection_id).await;
    }

    pub async fn remove_provider(&self, connection_id: &str) {
        let mut providers = self.providers.write().await;
        providers.remove(connection_id);
        self.clear_connection_error(connection_id).await;
    }

    pub async fn set_connection_error(&self, connection_id: &str, error: &str) {
        let mut errors = self.connection_errors.write().await;
        errors.insert(connection_id.to_string(), error.to_string());
    }

    pub async fn get_connection_error(&self, connection_id: &str) -> Option<String> {
        let errors = self.connection_errors.read().await;
        errors.get(connection_id).cloned()
    }

    pub async fn clear_connection_error(&self, connection_id: &str) {
        let mut errors = self.connection_errors.write().await;
        errors.remove(connection_id);
    }

    pub async fn get_system_setting(&self, key: &str) -> Option<String> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT value FROM system_settings WHERE key = ?",
        )
        .bind(key)
        .fetch_optional(&self.db)
        .await
        .unwrap_or(None);

        row.map(|r| r.0)
    }

    pub async fn set_system_setting(&self, key: &str, value: &str) -> anyhow::Result<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO system_settings (key, value, updated_at) VALUES (?, ?, ?)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        )
        .bind(key)
        .bind(value)
        .bind(&now)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    pub async fn get_user_preferences(&self, user_id: &str) -> Option<String> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT preferences_json FROM user_preferences WHERE user_id = ?",
        )
        .bind(user_id)
        .fetch_optional(&self.db)
        .await
        .unwrap_or(None);

        row.map(|r| r.0)
    }

    pub async fn set_user_preferences(&self, user_id: &str, preferences_json: &str) -> anyhow::Result<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO user_preferences (user_id, preferences_json, updated_at) VALUES (?, ?, ?)
             ON CONFLICT(user_id) DO UPDATE SET preferences_json = excluded.preferences_json, updated_at = excluded.updated_at",
        )
        .bind(user_id)
        .bind(preferences_json)
        .bind(&now)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    pub async fn update_local_root(&self, new_root: PathBuf, allow_symlinks: bool) -> anyhow::Result<()> {
        // Ensure root directory exists
        tokio::fs::create_dir_all(&new_root).await?;

        // Re-register local OpenDAL provider
        let root_str = new_root.to_string_lossy().to_string();
        let op = build_fs_operator(&root_str)?;
        let local_fs = Arc::new(OpenDalFileSystem::new_local("local", op, new_root.clone()));
        self.register_provider("local".to_string(), local_fs).await;

        // Persist to database
        self.set_system_setting("local_root", &root_str).await?;
        self.set_system_setting("allow_symlinks", if allow_symlinks { "true" } else { "false" }).await?;

        tracing::info!("Updated and persisted Local Storage root path to: {:?}", new_root);
        Ok(())
    }

    pub async fn load_providers_from_db(&self, master_key: &[u8; 32]) {
        // 1. Initialize Default Local Provider
        let local_root_setting = self.get_system_setting("local_root").await;
        let local_root = if let Some(custom_root) = local_root_setting {
            PathBuf::from(custom_root)
        } else {
            self.config.filesystem.default_local_root.clone()
        };

        if let Err(e) = tokio::fs::create_dir_all(&local_root).await {
            tracing::error!("Failed to create local root directory {:?}: {}", local_root, e);
            self.set_connection_error("local", &format!("Failed to create local directory: {}", e)).await;
        } else {
            let root_str = local_root.to_string_lossy().to_string();
            match build_fs_operator(&root_str) {
                Ok(op) => {
                    let local_fs = Arc::new(OpenDalFileSystem::new_local("local", op, local_root.clone()));
                    self.register_provider("local".to_string(), local_fs).await;
                    tracing::info!("Default Local Storage provider loaded at {:?}", local_root);
                }
                Err(e) => {
                    tracing::error!("Failed to init Local Storage provider: {}", e);
                    self.set_connection_error("local", &e.to_string()).await;
                }
            }
        }

        // 2. Query database for other enabled connections
        let rows: Vec<(
            String,
            String,
            String,
            Option<String>,
            Option<i64>,
            Option<String>,
            String,
        )> = sqlx::query_as(
            "SELECT id, name, provider, host, port, username, base_path FROM connections WHERE enabled = 1",
        )
        .fetch_all(&self.db)
        .await
        .unwrap_or_default();

        for (id, name, provider_type, host, port, username, base_path) in rows {
            if id == "local" {
                continue;
            }

            // Retrieve encrypted credential if exists
            let secret_row: Option<(String,)> = sqlx::query_as(
                "SELECT encrypted_secret FROM connection_credentials WHERE connection_id = ?",
            )
            .bind(&id)
            .fetch_optional(&self.db)
            .await
            .unwrap_or(None);

            let decrypted_secret = secret_row.and_then(|r| decrypt_secret(master_key, &r.0).ok());

            let build_result = match provider_type.as_str() {
                "ftp" => {
                    let host_str = host.unwrap_or_else(|| "127.0.0.1".into());
                    let port_num = port.unwrap_or(21) as u16;
                    build_ftp_operator(
                        &host_str,
                        port_num,
                        false,
                        username.as_deref(),
                        decrypted_secret.as_deref(),
                        Some(&base_path),
                    )
                }
                "ftps" => {
                    let host_str = host.unwrap_or_else(|| "127.0.0.1".into());
                    let port_num = port.unwrap_or(990) as u16;
                    build_ftp_operator(
                        &host_str,
                        port_num,
                        true,
                        username.as_deref(),
                        decrypted_secret.as_deref(),
                        Some(&base_path),
                    )
                }
                "sftp" => {
                    let host_str = host.unwrap_or_else(|| "127.0.0.1".into());
                    let port_num = port.unwrap_or(22) as u16;
                    let auth = decrypted_secret.map(|s| SftpAuth::Password { password: s });
                    build_sftp_operator(
                        &host_str,
                        port_num,
                        username.as_deref(),
                        auth.as_ref(),
                        Some(&base_path),
                    )
                }
                "s3" => {
                    let bucket = host.unwrap_or_else(|| "default-bucket".into());
                    build_s3_operator(
                        &bucket,
                        None,
                        None,
                        username.as_deref(),
                        decrypted_secret.as_deref(),
                        Some(&base_path),
                    )
                }
                other => {
                    tracing::warn!("Unsupported provider type '{}' for connection '{}'", other, id);
                    continue;
                }
            };

            match build_result {
                Ok(op) => {
                    let fs = Arc::new(OpenDalFileSystem::new(&id, op));
                    self.register_provider(id.clone(), fs).await;
                    tracing::info!("Storage connection '{}' ('{}', {}) initialized successfully", id, name, provider_type);
                }
                Err(e) => {
                    let err_msg = e.to_string();
                    tracing::error!("Failed to initialize storage connection '{}' ('{}', {}): {}", id, name, provider_type, err_msg);
                    self.set_connection_error(&id, &err_msg).await;
                }
            }
        }
    }
}
