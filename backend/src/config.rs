use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SecurityConfig {
    pub session_secret: String,
    pub session_ttl_secs: u64,
    pub allow_symlinks_outside_root: bool,
    pub allow_private_network_connections: bool,
    /// Comma-separated list of allowed CORS origins (e.g. "http://192.168.1.5:8080").
    /// Empty means mirror-request in dev, same-origin only in production.
    pub allowed_origins: Vec<String>,
    /// If true, set the Secure flag on session cookies regardless of host detection.
    /// If false (default), Secure is only set when the server host is not a loopback address.
    /// Set to false explicitly when serving over plain HTTP on LAN/Android.
    pub cookie_secure: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            session_secret: "dev_secret_change_in_production_32_chars_min".to_string(),
            session_ttl_secs: 86400 * 7,
            allow_symlinks_outside_root: false,
            allow_private_network_connections: true,
            allowed_origins: Vec::new(),
            cookie_secure: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FilesystemConfig {
    pub default_local_root: PathBuf,
    pub temp_dir: Option<PathBuf>,
    pub show_hidden_default: bool,
    pub read_only_default: bool,
}

impl Default for FilesystemConfig {
    fn default() -> Self {
        Self {
            default_local_root: PathBuf::from("./storage"),
            temp_dir: Some(PathBuf::from("./storage/temp")),
            show_hidden_default: false,
            read_only_default: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct LimitsConfig {
    pub max_upload_size: u64,
    pub max_editable_size: u64,
    pub max_preview_size: u64,
    pub max_directory_entries: usize,
    pub max_concurrent_transfers: usize,
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            max_upload_size: 1024 * 1024 * 1024,
            max_editable_size: 10 * 1024 * 1024,
            max_preview_size: 25 * 1024 * 1024,
            max_directory_entries: 50_000,
            max_concurrent_transfers: 4,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct DatabaseConfig {
    pub url: String,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "sqlite://./filemanager.db?mode=rwc".to_string(),
        }
    }
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

        if let Ok(hidden) = env::var("AEROFS_SHOW_HIDDEN") {
            self.filesystem.show_hidden_default = hidden == "1" || hidden.to_lowercase() == "true";
        }

        if let Ok(ro) = env::var("AEROFS_READ_ONLY") {
            self.filesystem.read_only_default = ro == "1" || ro.to_lowercase() == "true";
        }

        if let Ok(db_url) =
            env::var("AEROFS_DATABASE_URL").or_else(|_| env::var("WFM_DATABASE_URL"))
        {
            self.database.url = db_url;
        }

        if let Ok(symlinks) =
            env::var("AEROFS_ALLOW_SYMLINKS").or_else(|_| env::var("WFM_ALLOW_SYMLINKS"))
        {
            self.security.allow_symlinks_outside_root =
                symlinks == "1" || symlinks.to_lowercase() == "true";
        }

        if let Ok(private_net) = env::var("AEROFS_ALLOW_PRIVATE_NETWORKS") {
            self.security.allow_private_network_connections =
                private_net == "1" || private_net.to_lowercase() == "true";
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
        // Set to "false" or "0" when serving over plain HTTP (LAN / Android).
        if let Ok(val) = env::var("AEROFS_COOKIE_SECURE") {
            self.security.cookie_secure = val == "1" || val.to_lowercase() == "true";
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
}

fn dirs_config_path() -> PathBuf {
    if let Ok(home) = env::var("HOME") {
        PathBuf::from(home).join(".config/aerofs/config.toml")
    } else {
        PathBuf::from("./config.toml")
    }
}
