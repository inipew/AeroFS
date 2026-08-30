use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
#[allow(unused_imports)]
use std::time::Duration;
use thiserror::Error;

// ── Internal constants replacing magic numbers (67.md §127) ──
pub const DEFAULT_GLOBAL_IO_CONCURRENCY: usize = 32;
pub const DEFAULT_ARCHIVE_CONCURRENCY: usize = 4;
pub const DEFAULT_SEARCH_CONCURRENCY: usize = 8;
pub const DEFAULT_UPLOAD_PIPE_BUFFER: usize = 64 * 1024;
pub const STAGING_RETENTION_SECS: u64 = 24 * 3600;
pub const EVENT_JOURNAL_VACUUM_SECS: u64 = 6 * 3600;
pub const EVENT_JOURNAL_RETENTION_SECS: u64 = 24 * 3600;

// ── Env helpers (67.md §28) ──
fn env_bool(name: &str) -> Result<Option<bool>, ConfigError> {
    match env::var(name) {
        Ok(v) => {
            let lower = v.to_lowercase();
            match lower.as_str() {
                "1" | "true" | "yes" | "on" => Ok(Some(true)),
                "0" | "false" | "no" | "off" => Ok(Some(false)),
                _ => Err(ConfigError::Validation(format!(
                    "Invalid boolean for {}: '{}' (expected 1/0/true/false)",
                    name, v
                ))),
            }
        }
        Err(env::VarError::NotPresent) => Ok(None),
        Err(_) => Ok(None),
    }
}

fn env_parse<T: std::str::FromStr>(name: &str) -> Result<Option<T>, ConfigError>
where
    T::Err: std::fmt::Display,
{
    match env::var(name) {
        Ok(v) => v.parse::<T>().map(Some).map_err(|e| {
            ConfigError::Validation(format!("Invalid value for {}: '{}': {}", name, v, e))
        }),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(_) => Ok(None),
    }
}

fn env_var_with_fallback(primary: &str, fallback: &str) -> Option<String> {
    env::var(primary).or_else(|_| env::var(fallback)).ok()
}

fn env_var_with_fallbacks(primary: &str, fallbacks: &[&str]) -> Option<String> {
    if let Ok(v) = env::var(primary) {
        return Some(v);
    }
    for f in fallbacks {
        if let Ok(v) = env::var(*f) {
            return Some(v);
        }
    }
    None
}

pub mod database;
pub mod filesystem;
pub mod limits;
pub mod security;
pub mod server;
pub mod storage;

pub use database::DatabaseConfig;
pub use filesystem::FilesystemConfig;
pub use limits::LimitsConfig;
pub use security::SecurityConfig;
pub use server::ServerConfig;
pub use storage::{ProviderStorageConfig, StorageConfig};

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read configuration file at '{0}': {1}")]
    Io(PathBuf, #[source] std::io::Error),
    #[error("Failed to parse TOML configuration at '{0}': {1}")]
    Toml(PathBuf, #[source] toml::de::Error),
    #[error("Configuration validation error: {0}")]
    Validation(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub security: SecurityConfig,
    pub filesystem: FilesystemConfig,
    pub limits: LimitsConfig,
    pub database: DatabaseConfig,
    pub storage: StorageConfig,
}

impl AppConfig {
    /// Hierarchically load configuration:
    /// 1. Defaults
    /// 2. TOML file (CLI flag > env AEROFS_CONFIG > standard locations: /etc/aerofs/config.toml, ./aerofs.toml, ./config.toml)
    /// 3. Environment Variables (AEROFS_* and WFM_*)
    pub fn load(cli_config_path: Option<&Path>) -> Result<Self, ConfigError> {
        let mut config = Self::default();

        // 1. Determine config file path
        let explicit_path = cli_config_path.map(PathBuf::from).or_else(|| {
            env::var("AEROFS_CONFIG")
                .or_else(|_| env::var("WFM_CONFIG"))
                .ok()
                .map(PathBuf::from)
        });

        let toml_file_to_load = if let Some(path) = explicit_path {
            if !path.exists() {
                return Err(ConfigError::Io(
                    path.clone(),
                    std::io::Error::new(std::io::ErrorKind::NotFound, "Config file not found"),
                ));
            }
            Some(path)
        } else {
            // Search standard locations
            let candidates = [
                PathBuf::from("/etc/aerofs/config.toml"),
                dirs_config_path(),
                PathBuf::from("./aerofs.toml"),
                PathBuf::from("./config.toml"),
            ];
            candidates.into_iter().find(|p| p.exists())
        };

        // 2. If a TOML file was found, parse and overlay
        if let Some(path) = toml_file_to_load {
            let content =
                fs::read_to_string(&path).map_err(|e| ConfigError::Io(path.clone(), e))?;
            let toml_config: AppConfig =
                toml::from_str(&content).map_err(|e| ConfigError::Toml(path.clone(), e))?;
            config = toml_config;
            tracing::info!("Loaded configuration from: {}", path.display());
        }

        // 3. Override with Environment Variables
        config.apply_env_overrides()?;

        // 4. Validate configuration
        config.validate()?;

        Ok(config)
    }

    /// Apply environment variables with validation
    pub fn apply_env_overrides(&mut self) -> Result<(), ConfigError> {
        if let Ok(host) = env::var("AEROFS_HOST").or_else(|_| env::var("WFM_HOST")) {
            self.server.host = host;
        }

        if let Ok(port_str) = env::var("AEROFS_PORT").or_else(|_| env::var("WFM_PORT")) {
            let port = port_str.parse::<u16>().map_err(|_| {
                ConfigError::Validation(format!("Invalid integer for AEROFS_PORT: '{}'", port_str))
            })?;
            self.server.port = port;
        }

        if let Ok(root) = env::var("AEROFS_ROOT_PATH")
            .or_else(|_| env::var("WFM_ROOT_PATH"))
            .or_else(|_| env::var("WFM_LOCAL_ROOT"))
        {
            self.filesystem.default_local_root = PathBuf::from(root);
        }

        if let Ok(temp) = env::var("AEROFS_TEMP_DIR").or_else(|_| env::var("WFM_TEMP_DIR")) {
            self.filesystem.temp_dir = Some(PathBuf::from(temp));
        }

        if let Some(v) = env_bool("AEROFS_SHOW_HIDDEN")? {
            self.filesystem.show_hidden_default = v;
        }

        if let Some(v) = env_bool("AEROFS_READ_ONLY")? {
            self.filesystem.read_only_default = v;
        }

        if let Ok(db_url) =
            env::var("AEROFS_DATABASE_URL").or_else(|_| env::var("WFM_DATABASE_URL"))
        {
            self.database.url = db_url;
        }

        // Support legacy WFM_ prefix for symlinks (§29)
        if let Some(v) = env_bool("AEROFS_ALLOW_SYMLINKS")? {
            self.security.allow_symlinks_outside_root = v;
        } else if let Some(v) = env_bool("WFM_ALLOW_SYMLINKS")? {
            self.security.allow_symlinks_outside_root = v;
        }

        if let Some(v) = env_bool("AEROFS_ALLOW_PRIVATE_NETWORKS")? {
            self.security.allow_private_network_connections = v;
        }

        if let Ok(secret) =
            env::var("AEROFS_SESSION_SECRET").or_else(|_| env::var("WFM_SESSION_SECRET"))
        {
            self.security.session_secret = secret;
        }

        if let Ok(ttl_str) = env::var("AEROFS_SESSION_TTL") {
            let ttl = ttl_str.parse::<u64>().map_err(|_| {
                ConfigError::Validation(format!(
                    "Invalid integer for AEROFS_SESSION_TTL: '{}'",
                    ttl_str
                ))
            })?;
            self.security.session_ttl_secs = ttl;
        }

        // AEROFS_ALLOWED_ORIGINS: comma-separated list of allowed CORS origins.
        // Example: "http://192.168.1.5:8080,http://10.0.2.2:8080"
        if let Ok(origins_str) = env::var("AEROFS_ALLOWED_ORIGINS") {
            self.security.allowed_origins = origins_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }

        // AEROFS_COOKIE_SECURE: explicit control over the Secure cookie flag.
        if let Some(v) = env_bool("AEROFS_COOKIE_SECURE")? {
            self.security.cookie_secure = v;
        }

        if let Ok(trusted) = env::var("AEROFS_TRUSTED_PROXIES") {
            self.security.trusted_proxies = trusted
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }

        if let Ok(cred_key) = env::var("AEROFS_CREDENTIAL_ENCRYPTION_KEY")
            .or_else(|_| env::var("WFM_CREDENTIAL_ENCRYPTION_KEY"))
        {
            if !cred_key.trim().is_empty() {
                self.security.credential_encryption_key = Some(cred_key);
            }
        }

        if let Ok(max_upload_mb_str) =
            env::var("AEROFS_MAX_UPLOAD_MB").or_else(|_| env::var("WFM_MAX_UPLOAD_MB"))
        {
            let mb = max_upload_mb_str.parse::<u64>().map_err(|_| {
                ConfigError::Validation(format!(
                    "Invalid integer for AEROFS_MAX_UPLOAD_MB: '{}'",
                    max_upload_mb_str
                ))
            })?;
            let bytes = mb
                .checked_mul(1024)
                .and_then(|v| v.checked_mul(1024))
                .ok_or_else(|| {
                    ConfigError::Validation(
                        "AEROFS_MAX_UPLOAD_MB value causes integer overflow".to_string(),
                    )
                })?;
            self.limits.max_upload_size = bytes;
        }

        if let Ok(max_edit_mb_str) = env::var("AEROFS_MAX_EDITABLE_MB") {
            let mb = max_edit_mb_str.parse::<u64>().map_err(|_| {
                ConfigError::Validation(format!(
                    "Invalid integer for AEROFS_MAX_EDITABLE_MB: '{}'",
                    max_edit_mb_str
                ))
            })?;
            let bytes = mb
                .checked_mul(1024)
                .and_then(|v| v.checked_mul(1024))
                .ok_or_else(|| {
                    ConfigError::Validation(
                        "AEROFS_MAX_EDITABLE_MB value causes integer overflow".to_string(),
                    )
                })?;
            self.limits.max_editable_size = bytes;
        }

        if let Ok(max_prev_mb_str) = env::var("AEROFS_MAX_PREVIEW_MB") {
            let mb = max_prev_mb_str.parse::<u64>().map_err(|_| {
                ConfigError::Validation(format!(
                    "Invalid integer for AEROFS_MAX_PREVIEW_MB: '{}'",
                    max_prev_mb_str
                ))
            })?;
            let bytes = mb
                .checked_mul(1024)
                .and_then(|v| v.checked_mul(1024))
                .ok_or_else(|| {
                    ConfigError::Validation(
                        "AEROFS_MAX_PREVIEW_MB value causes integer overflow".to_string(),
                    )
                })?;
            self.limits.max_preview_size = bytes;
        }

        if let Ok(max_entries_str) = env::var("AEROFS_MAX_DIR_ENTRIES") {
            let entries = max_entries_str.parse::<usize>().map_err(|_| {
                ConfigError::Validation(format!(
                    "Invalid integer for AEROFS_MAX_DIR_ENTRIES: '{}'",
                    max_entries_str
                ))
            })?;
            self.limits.max_directory_entries = entries;
        }

        if let Ok(max_transfers_str) = env::var("AEROFS_MAX_TRANSFERS") {
            let n = max_transfers_str.parse::<usize>().map_err(|_| {
                ConfigError::Validation(format!(
                    "Invalid integer for AEROFS_MAX_TRANSFERS: '{}'",
                    max_transfers_str
                ))
            })?;
            self.limits.max_concurrent_transfers = n;
        }

        // Storage Layer Overrides
        if let Ok(c_str) = env::var("AEROFS_STORAGE_CONCURRENCY")
            .or_else(|_| env::var("AEROFS_STORAGE_DEFAULT_CONCURRENCY"))
        {
            if let Ok(c) = c_str.parse::<usize>() {
                self.storage.default_concurrency = c;
            }
        }
        if let Ok(t_str) = env::var("AEROFS_STORAGE_TIMEOUT")
            .or_else(|_| env::var("AEROFS_STORAGE_DEFAULT_TIMEOUT"))
        {
            if let Ok(t) = t_str.parse::<u64>() {
                self.storage.default_timeout_secs = t;
            }
        }
        if let Ok(iot_str) = env::var("AEROFS_STORAGE_IO_TIMEOUT")
            .or_else(|_| env::var("AEROFS_STORAGE_DEFAULT_IO_TIMEOUT"))
        {
            if let Ok(iot) = iot_str.parse::<u64>() {
                self.storage.default_io_timeout_secs = iot;
            }
        }
        if let Ok(s3_c_str) = env::var("AEROFS_STORAGE_S3_CONCURRENCY") {
            if let Ok(s3_c) = s3_c_str.parse::<usize>() {
                self.storage.s3.max_concurrency = s3_c;
            }
        }
        if let Ok(sftp_c_str) = env::var("AEROFS_STORAGE_SFTP_CONCURRENCY") {
            if let Ok(sftp_c) = sftp_c_str.parse::<usize>() {
                self.storage.sftp.max_concurrency = sftp_c;
            }
        }
        if let Ok(ftp_c_str) = env::var("AEROFS_STORAGE_FTP_CONCURRENCY") {
            if let Ok(ftp_c) = ftp_c_str.parse::<usize>() {
                self.storage.ftp.max_concurrency = ftp_c;
            }
        }

        Ok(())
    }

    /// Validate sanity of the configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.server.port == 0 {
            return Err(ConfigError::Validation("Server port cannot be 0".into()));
        }

        if self.database.url.is_empty() {
            return Err(ConfigError::Validation(
                "Database URL cannot be empty".into(),
            ));
        }

        if self.limits.max_upload_size == 0 {
            return Err(ConfigError::Validation(
                "Max upload size must be greater than 0".into(),
            ));
        }

        if self.limits.max_concurrent_transfers == 0 {
            return Err(ConfigError::Validation(
                "Max concurrent transfers must be at least 1".into(),
            ));
        }

        // Production environment check
        let is_prod = env::var("AEROFS_ENV")
            .map(|v| v.to_lowercase() == "production" || v.to_lowercase() == "prod")
            .unwrap_or(false);

        if is_prod {
            if self.security.session_secret == "dev_secret_change_in_production_32_chars_min" {
                return Err(ConfigError::Validation(
                    "Default development session secret is forbidden in production. Set AEROFS_SESSION_SECRET.".into(),
                ));
            }
            if self.security.session_secret.len() < 32 {
                return Err(ConfigError::Validation(
                    "Session secret must be at least 32 characters long in production.".into(),
                ));
            }
        }

        Ok(())
    }

    /// Return a sanitized copy with secrets masked for safe CLI display
    pub fn to_sanitized_toml(&self) -> String {
        let mut sanitized = self.clone();
        if !sanitized.security.session_secret.is_empty() {
            sanitized.security.session_secret = "********".to_string();
        }
        toml::to_string_pretty(&sanitized)
            .unwrap_or_else(|_| "# Error serializing config".to_string())
    }

    /// Inspect a single configuration key value by dotted path
    pub fn get_by_key_path(&self, key: &str) -> Option<String> {
        let key_lower = key.to_lowercase();
        match key_lower.as_str() {
            "server.host" | "host" => Some(self.server.host.clone()),
            "server.port" | "port" => Some(self.server.port.to_string()),
            "security.session_secret" | "session_secret" => Some("********".to_string()),
            "security.session_ttl_secs" | "session_ttl" => {
                Some(self.security.session_ttl_secs.to_string())
            }
            "security.allow_symlinks_outside_root" | "allow_symlinks" => {
                Some(self.security.allow_symlinks_outside_root.to_string())
            }
            "security.allow_private_network_connections" | "allow_private_networks" => {
                Some(self.security.allow_private_network_connections.to_string())
            }
            "security.allowed_origins" | "allowed_origins" => {
                Some(format!("{:?}", self.security.allowed_origins))
            }
            "security.cookie_secure" | "cookie_secure" => {
                Some(self.security.cookie_secure.to_string())
            }
            "filesystem.default_local_root" | "default_local_root" | "local_root" => {
                Some(self.filesystem.default_local_root.display().to_string())
            }
            "filesystem.temp_dir" | "temp_dir" => Some(
                self.filesystem
                    .temp_dir
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "None".to_string()),
            ),
            "filesystem.show_hidden_default" | "show_hidden" => {
                Some(self.filesystem.show_hidden_default.to_string())
            }
            "filesystem.read_only_default" | "read_only" => {
                Some(self.filesystem.read_only_default.to_string())
            }
            "limits.max_upload_size" | "max_upload_size" => Some(format!(
                "{} bytes ({} MB)",
                self.limits.max_upload_size,
                self.limits.max_upload_size / (1024 * 1024)
            )),
            "limits.max_editable_size" | "max_editable_size" => Some(format!(
                "{} bytes ({} MB)",
                self.limits.max_editable_size,
                self.limits.max_editable_size / (1024 * 1024)
            )),
            "limits.max_preview_size" | "max_preview_size" => Some(format!(
                "{} bytes ({} MB)",
                self.limits.max_preview_size,
                self.limits.max_preview_size / (1024 * 1024)
            )),
            "limits.max_directory_entries" | "max_directory_entries" => {
                Some(self.limits.max_directory_entries.to_string())
            }
            "limits.max_concurrent_transfers" | "max_concurrent_transfers" => {
                Some(self.limits.max_concurrent_transfers.to_string())
            }
            "database.url" | "database_url" => Some(crate::db::sanitize_db_url(&self.database.url)),
            _ => None,
        }
    }

    /// Retrieve provenance metadata for effective configuration layers
    pub fn get_effective_provenance(
        &self,
        config_path: Option<&Path>,
    ) -> Vec<ConfigProvenanceEntry> {
        let loaded_file = config_path
            .map(PathBuf::from)
            .or_else(|| {
                env::var("AEROFS_CONFIG")
                    .or_else(|_| env::var("WFM_CONFIG"))
                    .ok()
                    .map(PathBuf::from)
            })
            .or_else(|| {
                let candidates = [
                    PathBuf::from("/etc/aerofs/config.toml"),
                    dirs_config_path(),
                    PathBuf::from("./aerofs.toml"),
                    PathBuf::from("./config.toml"),
                ];
                candidates.into_iter().find(|p| p.exists())
            });

        let mut entries = Vec::new();

        // server.host
        let host_src = if env::var("AEROFS_HOST").is_ok() || env::var("WFM_HOST").is_ok() {
            ConfigSource::Environment("AEROFS_HOST".to_string())
        } else if let Some(ref f) = loaded_file {
            ConfigSource::ConfigFile(f.clone())
        } else {
            ConfigSource::Default
        };
        entries.push(ConfigProvenanceEntry {
            key: "server.host".to_string(),
            value: self.server.host.clone(),
            source: host_src,
            description: Some("Network host interface for HTTP/WebSocket listener".to_string()),
        });

        // server.port
        let port_src = if env::var("AEROFS_PORT").is_ok() || env::var("WFM_PORT").is_ok() {
            ConfigSource::Environment("AEROFS_PORT".to_string())
        } else if let Some(ref f) = loaded_file {
            ConfigSource::ConfigFile(f.clone())
        } else {
            ConfigSource::Default
        };
        entries.push(ConfigProvenanceEntry {
            key: "server.port".to_string(),
            value: self.server.port.to_string(),
            source: port_src,
            description: Some("TCP port for HTTP/WebSocket listener".to_string()),
        });

        // database.url
        let db_src =
            if env::var("AEROFS_DATABASE_URL").is_ok() || env::var("WFM_DATABASE_URL").is_ok() {
                ConfigSource::Environment("AEROFS_DATABASE_URL".to_string())
            } else if let Some(ref f) = loaded_file {
                ConfigSource::ConfigFile(f.clone())
            } else {
                ConfigSource::Default
            };
        entries.push(ConfigProvenanceEntry {
            key: "database.url".to_string(),
            value: crate::db::sanitize_db_url(&self.database.url),
            source: db_src,
            description: Some("SQLite database connection URL".to_string()),
        });

        // filesystem.default_local_root
        let root_src = if env::var("AEROFS_ROOT_PATH").is_ok()
            || env::var("WFM_ROOT_PATH").is_ok()
            || env::var("WFM_LOCAL_ROOT").is_ok()
        {
            ConfigSource::Environment("AEROFS_ROOT_PATH".to_string())
        } else if let Some(ref f) = loaded_file {
            ConfigSource::ConfigFile(f.clone())
        } else {
            ConfigSource::Default
        };
        entries.push(ConfigProvenanceEntry {
            key: "filesystem.default_local_root".to_string(),
            value: self.filesystem.default_local_root.display().to_string(),
            source: root_src,
            description: Some("Default local filesystem storage root path".to_string()),
        });

        // filesystem.temp_dir
        let temp_src = if env::var("AEROFS_TEMP_DIR").is_ok() || env::var("WFM_TEMP_DIR").is_ok() {
            ConfigSource::Environment("AEROFS_TEMP_DIR".to_string())
        } else if let Some(ref f) = loaded_file {
            ConfigSource::ConfigFile(f.clone())
        } else {
            ConfigSource::Default
        };
        entries.push(ConfigProvenanceEntry {
            key: "filesystem.temp_dir".to_string(),
            value: self
                .filesystem
                .temp_dir
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "None".to_string()),
            source: temp_src,
            description: Some(
                "Temporary directory for staging file uploads & archives".to_string(),
            ),
        });

        // limits.max_concurrent_transfers
        let trans_src = if env::var("AEROFS_MAX_TRANSFERS").is_ok() {
            ConfigSource::Environment("AEROFS_MAX_TRANSFERS".to_string())
        } else if let Some(ref f) = loaded_file {
            ConfigSource::ConfigFile(f.clone())
        } else {
            ConfigSource::Default
        };
        entries.push(ConfigProvenanceEntry {
            key: "limits.max_concurrent_transfers".to_string(),
            value: self.limits.max_concurrent_transfers.to_string(),
            source: trans_src,
            description: Some("Maximum simultaneous background transfer workers".to_string()),
        });

        // limits.max_upload_size
        let upload_src =
            if env::var("AEROFS_MAX_UPLOAD_MB").is_ok() || env::var("WFM_MAX_UPLOAD_MB").is_ok() {
                ConfigSource::Environment("AEROFS_MAX_UPLOAD_MB".to_string())
            } else if let Some(ref f) = loaded_file {
                ConfigSource::ConfigFile(f.clone())
            } else {
                ConfigSource::Default
            };
        entries.push(ConfigProvenanceEntry {
            key: "limits.max_upload_size".to_string(),
            value: format!(
                "{} bytes ({} MB)",
                self.limits.max_upload_size,
                self.limits.max_upload_size / (1024 * 1024)
            ),
            source: upload_src,
            description: Some("Maximum allowed size per uploaded file".to_string()),
        });

        // security.session_ttl_secs
        let ttl_src = if env::var("AEROFS_SESSION_TTL").is_ok() {
            ConfigSource::Environment("AEROFS_SESSION_TTL".to_string())
        } else if let Some(ref f) = loaded_file {
            ConfigSource::ConfigFile(f.clone())
        } else {
            ConfigSource::Default
        };
        entries.push(ConfigProvenanceEntry {
            key: "security.session_ttl_secs".to_string(),
            value: format!(
                "{}s ({} hours)",
                self.security.session_ttl_secs,
                self.security.session_ttl_secs / 3600
            ),
            source: ttl_src,
            description: Some("Session expiration time in seconds".to_string()),
        });

        // security.allow_symlinks_outside_root
        let sym_src = if env::var("AEROFS_ALLOW_SYMLINKS").is_ok() {
            ConfigSource::Environment("AEROFS_ALLOW_SYMLINKS".to_string())
        } else if let Some(ref f) = loaded_file {
            ConfigSource::ConfigFile(f.clone())
        } else {
            ConfigSource::Default
        };
        entries.push(ConfigProvenanceEntry {
            key: "security.allow_symlinks_outside_root".to_string(),
            value: self.security.allow_symlinks_outside_root.to_string(),
            source: sym_src,
            description: Some(
                "Whether symlinks pointing outside storage root are resolved".to_string(),
            ),
        });

        // security.allow_private_network_connections
        let priv_src = if env::var("AEROFS_ALLOW_PRIVATE_NETWORKS").is_ok() {
            ConfigSource::Environment("AEROFS_ALLOW_PRIVATE_NETWORKS".to_string())
        } else if let Some(ref f) = loaded_file {
            ConfigSource::ConfigFile(f.clone())
        } else {
            ConfigSource::Default
        };
        entries.push(ConfigProvenanceEntry {
            key: "security.allow_private_network_connections".to_string(),
            value: self.security.allow_private_network_connections.to_string(),
            source: priv_src,
            description: Some(
                "Allow remote connections in private RFC1918 / loopback networks".to_string(),
            ),
        });

        // limits.max_editable_size
        let edit_src = if env::var("AEROFS_MAX_EDITABLE_MB").is_ok() {
            ConfigSource::Environment("AEROFS_MAX_EDITABLE_MB".to_string())
        } else if let Some(ref f) = loaded_file {
            ConfigSource::ConfigFile(f.clone())
        } else {
            ConfigSource::Default
        };
        entries.push(ConfigProvenanceEntry {
            key: "limits.max_editable_size".to_string(),
            value: format!(
                "{} bytes ({} MB)",
                self.limits.max_editable_size,
                self.limits.max_editable_size / (1024 * 1024)
            ),
            source: edit_src,
            description: Some("Maximum file size allowed for in-browser editor".to_string()),
        });

        // limits.max_directory_entries
        let dir_src = if env::var("AEROFS_MAX_DIR_ENTRIES").is_ok() {
            ConfigSource::Environment("AEROFS_MAX_DIR_ENTRIES".to_string())
        } else if let Some(ref f) = loaded_file {
            ConfigSource::ConfigFile(f.clone())
        } else {
            ConfigSource::Default
        };
        entries.push(ConfigProvenanceEntry {
            key: "limits.max_directory_entries".to_string(),
            value: self.limits.max_directory_entries.to_string(),
            source: dir_src,
            description: Some("Maximum items returned per directory listing".to_string()),
        });

        // security.session_secret
        let secret_src = if env::var("AEROFS_SESSION_SECRET").is_ok()
            || env::var("WFM_SESSION_SECRET").is_ok()
        {
            ConfigSource::Environment("AEROFS_SESSION_SECRET".to_string())
        } else if let Some(ref f) = loaded_file {
            ConfigSource::ConfigFile(f.clone())
        } else {
            ConfigSource::Default
        };
        entries.push(ConfigProvenanceEntry {
            key: "security.session_secret".to_string(),
            value: "********".to_string(),
            source: secret_src,
            description: Some(
                "HMAC cryptographic key used for cookie signing & credential encryption"
                    .to_string(),
            ),
        });

        entries
    }

    /// Describe metadata for a specific configuration key
    pub fn describe_key(key: &str) -> Option<ConfigDescriptor> {
        let key_lower = key.to_lowercase();
        CONFIG_DESCRIPTORS
            .iter()
            .find(|d| d.key.eq_ignore_ascii_case(&key_lower))
            .copied()
    }

    /// Return all registered configuration descriptors
    pub fn describe_all() -> &'static [ConfigDescriptor] {
        CONFIG_DESCRIPTORS
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConfigSource {
    Default,
    ConfigFile(PathBuf),
    Environment(String),
    CliOverride,
}

impl std::fmt::Display for ConfigSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigSource::Default => write!(f, "default"),
            ConfigSource::ConfigFile(p) => write!(f, "config file ({})", p.display()),
            ConfigSource::Environment(env_var) => write!(f, "environment variable ({})", env_var),
            ConfigSource::CliOverride => write!(f, "CLI flag"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigProvenanceEntry {
    pub key: String,
    pub value: String,
    pub source: ConfigSource,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ConfigDescriptor {
    pub key: &'static str,
    pub description: &'static str,
    pub value_type: &'static str,
    pub default_value: &'static str,
    pub env_variable: Option<&'static str>,
    pub runtime_mutable: bool,
    pub restart_required: bool,
    pub subsystems: &'static [&'static str],
}

pub static CONFIG_DESCRIPTORS: &[ConfigDescriptor] = &[
    ConfigDescriptor {
        key: "server.host",
        description: "Network host/IP address to bind the HTTP and WebSocket listeners to",
        value_type: "string",
        default_value: "127.0.0.1",
        env_variable: Some("AEROFS_HOST"),
        runtime_mutable: false,
        restart_required: true,
        subsystems: &["HTTP Server", "WebSocket"],
    },
    ConfigDescriptor {
        key: "server.port",
        description: "TCP port number to bind the HTTP and WebSocket listeners to",
        value_type: "u16",
        default_value: "8080",
        env_variable: Some("AEROFS_PORT"),
        runtime_mutable: false,
        restart_required: true,
        subsystems: &["HTTP Server", "WebSocket"],
    },
    ConfigDescriptor {
        key: "database.url",
        description: "Database connection URL (SQLite with WAL mode and foreign key constraints)",
        value_type: "string",
        default_value: "sqlite://./filemanager.db?mode=rwc",
        env_variable: Some("AEROFS_DATABASE_URL"),
        runtime_mutable: false,
        restart_required: true,
        subsystems: &["Database", "Sessions", "Transfers", "Permissions"],
    },
    ConfigDescriptor {
        key: "security.session_secret",
        description: "Cryptographic HMAC secret used to sign session cookies and encrypt remote connection credentials",
        value_type: "string (min 32 chars)",
        default_value: "dev_secret_change_in_production_32_chars_min",
        env_variable: Some("AEROFS_SESSION_SECRET"),
        runtime_mutable: false,
        restart_required: true,
        subsystems: &["Auth", "Session Middleware", "Credential Store"],
    },
    ConfigDescriptor {
        key: "security.session_ttl_secs",
        description: "Session expiration time in seconds (default: 7 days)",
        value_type: "u64",
        default_value: "604800",
        env_variable: Some("AEROFS_SESSION_TTL"),
        runtime_mutable: true,
        restart_required: false,
        subsystems: &["Auth", "Session Middleware"],
    },
    ConfigDescriptor {
        key: "security.allow_symlinks_outside_root",
        description: "Whether symlinks pointing outside the storage root directory are allowed to be resolved",
        value_type: "bool",
        default_value: "false",
        env_variable: Some("AEROFS_ALLOW_SYMLINKS"),
        runtime_mutable: false,
        restart_required: true,
        subsystems: &["VFS", "SafePath"],
    },
    ConfigDescriptor {
        key: "security.allow_private_network_connections",
        description: "Allow connecting to remote storage providers residing in private RFC1918 / loopback networks",
        value_type: "bool",
        default_value: "true",
        env_variable: Some("AEROFS_ALLOW_PRIVATE_NETWORKS"),
        runtime_mutable: false,
        restart_required: true,
        subsystems: &["OpenDAL Providers", "SSRF Protection"],
    },
    ConfigDescriptor {
        key: "security.cookie_secure",
        description: "Explicitly force the Secure flag on authentication session cookies",
        value_type: "bool",
        default_value: "false",
        env_variable: Some("AEROFS_COOKIE_SECURE"),
        runtime_mutable: false,
        restart_required: true,
        subsystems: &["Auth", "Cookies"],
    },
    ConfigDescriptor {
        key: "filesystem.default_local_root",
        description: "Local directory on the filesystem served as the primary storage root",
        value_type: "PathBuf",
        default_value: "./storage",
        env_variable: Some("AEROFS_ROOT_PATH"),
        runtime_mutable: false,
        restart_required: true,
        subsystems: &["VFS", "Local Provider"],
    },
    ConfigDescriptor {
        key: "filesystem.temp_dir",
        description: "Directory used for temporary files, chunked upload staging, and archive extraction",
        value_type: "Option<PathBuf>",
        default_value: "./storage/temp",
        env_variable: Some("AEROFS_TEMP_DIR"),
        runtime_mutable: false,
        restart_required: true,
        subsystems: &["Multipart Upload", "Archive Engine"],
    },
    ConfigDescriptor {
        key: "limits.max_upload_size",
        description: "Maximum size in bytes allowed for a single file upload",
        value_type: "u64 (bytes)",
        default_value: "1073741824 (1 GB)",
        env_variable: Some("AEROFS_MAX_UPLOAD_MB"),
        runtime_mutable: true,
        restart_required: false,
        subsystems: &["Upload Engine", "Transfer Manager"],
    },
    ConfigDescriptor {
        key: "limits.max_concurrent_transfers",
        description: "Maximum simultaneous background file transfer worker tasks",
        value_type: "usize",
        default_value: "4",
        env_variable: Some("AEROFS_MAX_TRANSFERS"),
        runtime_mutable: false,
        restart_required: true,
        subsystems: &["TransferManager", "TransferScheduler"],
    },
    ConfigDescriptor {
        key: "limits.max_editable_size",
        description: "Maximum file size in bytes allowed for in-browser text/code editor",
        value_type: "u64 (bytes)",
        default_value: "10485760 (10 MB)",
        env_variable: Some("AEROFS_MAX_EDITABLE_MB"),
        runtime_mutable: true,
        restart_required: false,
        subsystems: &["Editor Service"],
    },
    ConfigDescriptor {
        key: "limits.max_directory_entries",
        description: "Maximum number of directory entries returned per directory listing call",
        value_type: "usize",
        default_value: "50000",
        env_variable: Some("AEROFS_MAX_DIR_ENTRIES"),
        runtime_mutable: true,
        restart_required: false,
        subsystems: &["VFS", "Directory Listing"],
    },
    ConfigDescriptor {
        key: "storage.default_concurrency",
        description: "Default concurrency limit across OpenDAL storage backends",
        value_type: "usize",
        default_value: "16",
        env_variable: Some("AEROFS_STORAGE_CONCURRENCY"),
        runtime_mutable: true,
        restart_required: false,
        subsystems: &["OpenDAL", "Storage Runtime"],
    },
    ConfigDescriptor {
        key: "storage.default_timeout_secs",
        description: "Default control operation timeout for storage provider requests in seconds",
        value_type: "u64",
        default_value: "60",
        env_variable: Some("AEROFS_STORAGE_TIMEOUT"),
        runtime_mutable: true,
        restart_required: false,
        subsystems: &["OpenDAL", "Storage Runtime"],
    },
    ConfigDescriptor {
        key: "storage.s3.max_concurrency",
        description: "Maximum concurrent in-flight requests for Amazon S3 / S3-compatible endpoints",
        value_type: "usize",
        default_value: "64",
        env_variable: Some("AEROFS_STORAGE_S3_CONCURRENCY"),
        runtime_mutable: true,
        restart_required: false,
        subsystems: &["OpenDAL", "S3 Provider"],
    },
    ConfigDescriptor {
        key: "storage.sftp.max_concurrency",
        description: "Maximum concurrent in-flight requests for remote SFTP / SSH connections",
        value_type: "usize",
        default_value: "8",
        env_variable: Some("AEROFS_STORAGE_SFTP_CONCURRENCY"),
        runtime_mutable: true,
        restart_required: false,
        subsystems: &["OpenDAL", "SFTP Provider"],
    },
];

fn dirs_config_path() -> PathBuf {
    if let Ok(home) = env::var("HOME") {
        PathBuf::from(home).join(".config/aerofs/config.toml")
    } else {
        PathBuf::from("./config.toml")
    }
}
