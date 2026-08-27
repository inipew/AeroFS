use crate::auth::credentials::{decrypt_secret, derive_master_key};
use crate::config::AppConfig;
use crate::db::DbPool;
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
    pub transfer_manager: TransferManager,
}

impl AppState {
    pub async fn new_with_db(config: AppConfig, db: DbPool) -> Self {
        // Ensure system_settings table exists
        let _ = sqlx::query(
            "CREATE TABLE IF NOT EXISTS system_settings (
                key TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&db)
        .await;

        let master_key = derive_master_key(&config.security.session_secret);
        let providers_map = Arc::new(RwLock::new(HashMap::new()));
        let transfer_manager = TransferManager::new(Arc::clone(&providers_map), db.clone());

        let state = Self {
            config: Arc::new(config),
            db,
            providers: providers_map,
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
        providers.insert(connection_id, provider);
    }

    pub async fn remove_provider(&self, connection_id: &str) {
        let mut providers = self.providers.write().await;
        providers.remove(connection_id);
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

    pub async fn update_local_root(&self, new_root: PathBuf, allow_symlinks: bool) -> anyhow::Result<()> {
        // Ensure root directory exists
        tokio::fs::create_dir_all(&new_root).await?;

        // Re-register local OpenDAL provider
        let root_str = new_root.to_string_lossy().to_string();
        let op = build_fs_operator(&root_str)?;
        let local_fs = Arc::new(OpenDalFileSystem::new("local", op));
        self.register_provider("local".to_string(), local_fs).await;

        // Persist to database
        self.set_system_setting("local_root", &root_str).await?;
        self.set_system_setting("allow_symlinks", if allow_symlinks { "true" } else { "false" }).await?;

        tracing::info!("Updated and persisted Local Storage root path to: {:?}", new_root);
        Ok(())
    }

    async fn load_providers_from_db(&self, master_key: &[u8; 32]) {
        // Retrieve custom local root from DB if exists, else fallback to config
        let local_root = if let Some(custom_root) = self.get_system_setting("local_root").await {
            PathBuf::from(custom_root)
        } else {
            self.config.filesystem.default_local_root.clone()
        };

        // Ensure directory exists
        let _ = tokio::fs::create_dir_all(&local_root).await;

        // Register default local OpenDAL provider
        let root_str = local_root.to_string_lossy().to_string();
        if let Ok(op) = build_fs_operator(&root_str) {
            let local_fs = Arc::new(OpenDalFileSystem::new("local", op));
            self.register_provider("local".to_string(), local_fs).await;
        }

        // Query database for other enabled connections
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

        for (id, _name, provider_type, host, port, username, base_path) in rows {
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

            match provider_type.as_str() {
                "ftp" => {
                    let host_str = host.unwrap_or_else(|| "127.0.0.1".into());
                    let port_num = port.unwrap_or(21) as u16;
                    if let Ok(op) = build_ftp_operator(
                        &host_str,
                        port_num,
                        false,
                        username.as_deref(),
                        decrypted_secret.as_deref(),
                        Some(&base_path),
                    ) {
                        let fs = Arc::new(OpenDalFileSystem::new(&id, op));
                        self.register_provider(id, fs).await;
                    }
                }
                "ftps" => {
                    let host_str = host.unwrap_or_else(|| "127.0.0.1".into());
                    let port_num = port.unwrap_or(990) as u16;
                    if let Ok(op) = build_ftp_operator(
                        &host_str,
                        port_num,
                        true,
                        username.as_deref(),
                        decrypted_secret.as_deref(),
                        Some(&base_path),
                    ) {
                        let fs = Arc::new(OpenDalFileSystem::new(&id, op));
                        self.register_provider(id, fs).await;
                    }
                }
                "sftp" => {
                    let host_str = host.unwrap_or_else(|| "127.0.0.1".into());
                    let port_num = port.unwrap_or(22) as u16;
                    if let Ok(op) = build_sftp_operator(
                        &host_str,
                        port_num,
                        username.as_deref(),
                        decrypted_secret.as_deref(),
                        Some(&base_path),
                    ) {
                        let fs = Arc::new(OpenDalFileSystem::new(&id, op));
                        self.register_provider(id, fs).await;
                    }
                }
                "s3" => {
                    let bucket = host.unwrap_or_else(|| "default-bucket".into());
                    if let Ok(op) = build_s3_operator(
                        &bucket,
                        None,
                        None,
                        username.as_deref(),
                        decrypted_secret.as_deref(),
                        Some(&base_path),
                    ) {
                        let fs = Arc::new(OpenDalFileSystem::new(&id, op));
                        self.register_provider(id, fs).await;
                    }
                }
                _ => {}
            }
        }
    }
}
