use crate::auth::audit::record_audit_log;
use crate::config::AppConfig;
use crate::db::DbPool;
use crate::domain::{settings::*, Actor};
use crate::errors::AppError;
use crate::transfer::TransferManager;
use crate::vfs::{factory::ProviderFactory, registry::ProviderRegistry};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct SettingsResponse {
    pub settings: AppSettings,
    pub database_url: String,
    pub max_upload_mb: u64,
    pub max_editable_size: u64,
    pub local_root: String,
    pub temp_dir: String,
    pub allow_symlinks: bool,
    pub show_hidden_default: bool,
    pub read_only_default: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateSettingsRequest {
    pub settings: Option<AppSettings>,
    pub local_root: Option<String>,
    pub temp_dir: Option<String>,
    pub allow_symlinks: Option<bool>,
    pub show_hidden_default: Option<bool>,
    pub read_only_default: Option<bool>,
}

async fn upsert_setting(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    key: &str,
    val: &str,
    now: &str,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO system_settings (key, value, updated_at) VALUES (?, ?, ?)\n         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
    )
    .bind(key)
    .bind(val)
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(|e| anyhow::anyhow!("DB error: {}", e))?;
    Ok(())
}

#[derive(Clone)]
pub struct SettingsService {
    db: DbPool,
    config: Arc<AppConfig>,
    registry: Arc<ProviderRegistry>,
    transfer_manager: TransferManager,
}

impl SettingsService {
    pub fn new(
        db: DbPool,
        config: Arc<AppConfig>,
        registry: Arc<ProviderRegistry>,
        transfer_manager: TransferManager,
    ) -> Self {
        Self {
            db,
            config,
            registry,
            transfer_manager,
        }
    }

    pub async fn get_system_setting(&self, key: &str) -> Option<String> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT value FROM system_settings WHERE key = ?")
                .bind(key)
                .fetch_optional(&self.db)
                .await
                .unwrap_or(None);

        row.map(|r| r.0)
    }

    pub async fn set_system_setting(&self, key: &str, value: &str) -> anyhow::Result<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO system_settings (key, value, updated_at) VALUES (?, ?, ?)\n             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        )
        .bind(key)
        .bind(value)
        .bind(&now)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    async fn refresh_local_root_runtime(&self, new_root: PathBuf) -> anyhow::Result<()> {
        tokio::fs::create_dir_all(&new_root).await?;
        let local_fs = ProviderFactory::build_local("local", new_root)?;
        self.registry.register("local".to_string(), local_fs).await;
        Ok(())
    }

    pub async fn update_local_root(
        &self,
        new_root: PathBuf,
        _allow_symlinks: bool,
    ) -> anyhow::Result<()> {
        self.refresh_local_root_runtime(new_root.clone()).await?;
        self.set_system_setting("local_root", &new_root.to_string_lossy())
            .await
    }

    pub async fn get_settings(&self, _actor: &Actor) -> Result<SettingsResponse, AppError> {
        let local_root = if let Some(custom) = self.get_system_setting("local_root").await {
            custom
        } else {
            self.config
                .filesystem
                .default_local_root
                .to_string_lossy()
                .to_string()
        };

        let temp_dir = if let Some(custom) = self.get_system_setting("temp_dir").await {
            custom
        } else {
            self.config
                .filesystem
                .temp_dir
                .as_ref()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "./storage/temp".to_string())
        };

        let allow_symlinks = if let Some(val) = self.get_system_setting("allow_symlinks").await {
            val == "true"
        } else {
            self.config.security.allow_symlinks_outside_root
        };

        let show_hidden_default =
            if let Some(val) = self.get_system_setting("show_hidden_default").await {
                val == "true"
            } else {
                self.config.filesystem.show_hidden_default
            };

        let read_only_default =
            if let Some(val) = self.get_system_setting("read_only_default").await {
                val == "true"
            } else {
                self.config.filesystem.read_only_default
            };

        let max_editable_size = self
            .get_system_setting("max_editable_size")
            .await
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(self.config.limits.max_editable_size);

        let theme = self
            .get_system_setting("theme")
            .await
            .unwrap_or_else(|| "dark".to_string());
        let default_view = self
            .get_system_setting("default_view")
            .await
            .unwrap_or_else(|| "grid".to_string());
        let default_layout = self
            .get_system_setting("default_layout")
            .await
            .unwrap_or_else(|| "split".to_string());
        let max_transfers = self
            .get_system_setting("max_concurrent_transfers")
            .await
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(self.config.limits.max_concurrent_transfers);

        let settings = AppSettings {
            general: GeneralSettings {
                language: "en".to_string(),
                theme,
                default_view,
                default_sort: "name".to_string(),
                sort_direction: "asc".to_string(),
                show_hidden_default,
                confirm_destructive: true,
            },
            file_manager: FileManagerSettings {
                default_layout,
                show_breadcrumbs: true,
                show_file_size: true,
                show_permissions: true,
                remember_last_directories: true,
                max_editable_size,
            },
            transfers: TransferSettings {
                max_concurrent_transfers: max_transfers,
                retry_attempts: 3,
                auto_retry: true,
                show_notifications: true,
            },
            connections: ConnectionSettings {
                connection_timeout_secs: 60,
                health_check_interval_secs: 30,
                auto_reconnect: true,
                default_local_root: local_root.clone(),
                temp_dir: temp_dir.clone(),
            },
            security: SecuritySettings {
                allow_symlinks_outside_root: allow_symlinks,
                confirm_permanent_delete: true,
                read_only_default,
                session_timeout_secs: 86400,
            },
            advanced: AdvancedSettings {
                log_level: "info".to_string(),
                enable_telemetry: true,
                enable_tracing: true,
                directory_cache_ttl_secs: 0,
            },
        };

        Ok(SettingsResponse {
            settings,
            database_url: "sqlite://...".to_string(),
            max_upload_mb: self.config.limits.max_upload_size / (1024 * 1024),
            max_editable_size,
            local_root,
            temp_dir,
            allow_symlinks,
            show_hidden_default,
            read_only_default,
        })
    }

    pub async fn update_settings(
        &self,
        actor: &Actor,
        payload: UpdateSettingsRequest,
    ) -> Result<(), AppError> {
        if !actor.is_admin {
            return Err(AppError::Forbidden(
                "Only administrators can update system settings".into(),
            ));
        }

        let target_root = payload
            .local_root
            .as_deref()
            .or_else(|| {
                payload
                    .settings
                    .as_ref()
                    .map(|s| s.connections.default_local_root.as_str())
            })
            .filter(|s| !s.trim().is_empty())
            .map(PathBuf::from);

        let mut tx = self
            .db
            .begin()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to begin settings transaction: {}", e))?;

        let now = Utc::now().to_rfc3339();

        if let Some(app_settings) = &payload.settings {
            upsert_setting(
                &mut tx,
                "local_root",
                &app_settings.connections.default_local_root,
                &now,
            )
            .await?;
            upsert_setting(
                &mut tx,
                "temp_dir",
                &app_settings.connections.temp_dir,
                &now,
            )
            .await?;
            upsert_setting(
                &mut tx,
                "allow_symlinks",
                if app_settings.security.allow_symlinks_outside_root {
                    "true"
                } else {
                    "false"
                },
                &now,
            )
            .await?;
            upsert_setting(
                &mut tx,
                "show_hidden_default",
                if app_settings.general.show_hidden_default {
                    "true"
                } else {
                    "false"
                },
                &now,
            )
            .await?;
            upsert_setting(
                &mut tx,
                "read_only_default",
                if app_settings.security.read_only_default {
                    "true"
                } else {
                    "false"
                },
                &now,
            )
            .await?;
            upsert_setting(
                &mut tx,
                "max_editable_size",
                &app_settings.file_manager.max_editable_size.to_string(),
                &now,
            )
            .await?;
            upsert_setting(
                &mut tx,
                "max_concurrent_transfers",
                &app_settings.transfers.max_concurrent_transfers.to_string(),
                &now,
            )
            .await?;
            upsert_setting(&mut tx, "theme", &app_settings.general.theme, &now).await?;
            upsert_setting(
                &mut tx,
                "default_view",
                &app_settings.general.default_view,
                &now,
            )
            .await?;
            upsert_setting(
                &mut tx,
                "default_layout",
                &app_settings.file_manager.default_layout,
                &now,
            )
            .await?;
        }

        if let Some(lr) = &payload.local_root {
            upsert_setting(&mut tx, "local_root", lr, &now).await?;
        }
        if let Some(td) = &payload.temp_dir {
            upsert_setting(&mut tx, "temp_dir", td, &now).await?;
        }
        if let Some(sym) = payload.allow_symlinks {
            upsert_setting(
                &mut tx,
                "allow_symlinks",
                if sym { "true" } else { "false" },
                &now,
            )
            .await?;
        }
        if let Some(sh) = payload.show_hidden_default {
            upsert_setting(
                &mut tx,
                "show_hidden_default",
                if sh { "true" } else { "false" },
                &now,
            )
            .await?;
        }
        if let Some(ro) = payload.read_only_default {
            upsert_setting(
                &mut tx,
                "read_only_default",
                if ro { "true" } else { "false" },
                &now,
            )
            .await?;
        }

        tx.commit()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to commit settings transaction: {}", e))?;

        if let Some(root_path) = target_root {
            if let Err(error) = self.refresh_local_root_runtime(root_path).await {
                tracing::warn!(%error, "failed to refresh local provider after settings update");
            }
        }

        if let Some(app_settings) = &payload.settings {
            self.transfer_manager
                .set_max_concurrent_transfers(app_settings.transfers.max_concurrent_transfers);
        }

        record_audit_log(
            &self.db,
            Some(&actor.id),
            "SETTINGS_UPDATED",
            None,
            None,
            "success",
            None,
            Some("Updated system settings"),
        )
        .await;

        Ok(())
    }
}
