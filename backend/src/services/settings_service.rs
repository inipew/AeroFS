use crate::config::AppConfig;
use crate::domain::{settings::*, Actor};
use crate::errors::AppError;
use crate::ports::settings::{SettingsAudit, SettingsRuntime, SystemSettingsStore};
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

#[derive(Clone)]
pub struct SettingsService {
    config: Arc<AppConfig>,
    store: Arc<dyn SystemSettingsStore>,
    runtime: Arc<dyn SettingsRuntime>,
    audit: Arc<dyn SettingsAudit>,
}

impl SettingsService {
    pub fn new(
        config: Arc<AppConfig>,
        store: Arc<dyn SystemSettingsStore>,
        runtime: Arc<dyn SettingsRuntime>,
        audit: Arc<dyn SettingsAudit>,
    ) -> Self {
        Self {
            config,
            store,
            runtime,
            audit,
        }
    }

    pub async fn get_system_setting(&self, key: &str) -> Option<String> {
        match self.store.get(key).await {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(%error, key, "failed to read system setting");
                None
            }
        }
    }

    pub async fn set_system_setting(&self, key: &str, value: &str) -> anyhow::Result<()> {
        self.store
            .upsert_many(&[(key.to_string(), value.to_string())])
            .await
            .map_err(|error| anyhow::anyhow!(error.to_string()))
    }

    pub async fn update_local_root(
        &self,
        new_root: PathBuf,
        _allow_symlinks: bool,
    ) -> anyhow::Result<()> {
        let prepared = self
            .runtime
            .prepare_local_root(new_root.clone())
            .await
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        self.store
            .upsert_many(&[(
                "local_root".to_string(),
                new_root.to_string_lossy().to_string(),
            )])
            .await
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        prepared.activate().await;
        Ok(())
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

        let prepared_root = match target_root {
            Some(root) => Some(self.runtime.prepare_local_root(root).await?),
            None => None,
        };

        let mut values = Vec::new();
        if let Some(app_settings) = &payload.settings {
            values.extend([
                (
                    "local_root".to_string(),
                    app_settings.connections.default_local_root.clone(),
                ),
                (
                    "temp_dir".to_string(),
                    app_settings.connections.temp_dir.clone(),
                ),
                (
                    "allow_symlinks".to_string(),
                    app_settings
                        .security
                        .allow_symlinks_outside_root
                        .to_string(),
                ),
                (
                    "show_hidden_default".to_string(),
                    app_settings.general.show_hidden_default.to_string(),
                ),
                (
                    "read_only_default".to_string(),
                    app_settings.security.read_only_default.to_string(),
                ),
                (
                    "max_editable_size".to_string(),
                    app_settings.file_manager.max_editable_size.to_string(),
                ),
                (
                    "max_concurrent_transfers".to_string(),
                    app_settings.transfers.max_concurrent_transfers.to_string(),
                ),
                ("theme".to_string(), app_settings.general.theme.clone()),
                (
                    "default_view".to_string(),
                    app_settings.general.default_view.clone(),
                ),
                (
                    "default_layout".to_string(),
                    app_settings.file_manager.default_layout.clone(),
                ),
            ]);
        }

        if let Some(value) = &payload.local_root {
            values.push(("local_root".to_string(), value.clone()));
        }
        if let Some(value) = &payload.temp_dir {
            values.push(("temp_dir".to_string(), value.clone()));
        }
        if let Some(value) = payload.allow_symlinks {
            values.push(("allow_symlinks".to_string(), value.to_string()));
        }
        if let Some(value) = payload.show_hidden_default {
            values.push(("show_hidden_default".to_string(), value.to_string()));
        }
        if let Some(value) = payload.read_only_default {
            values.push(("read_only_default".to_string(), value.to_string()));
        }

        self.store.upsert_many(&values).await?;

        if let Some(prepared) = prepared_root {
            prepared.activate().await;
        }

        if let Some(app_settings) = &payload.settings {
            self.runtime
                .set_max_concurrent_transfers(app_settings.transfers.max_concurrent_transfers);
        }

        self.audit.settings_updated(actor).await;
        Ok(())
    }
}
