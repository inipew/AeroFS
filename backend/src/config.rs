use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub security: SecurityConfig,
    pub filesystem: FilesystemConfig,
    pub limits: LimitsConfig,
    pub database: DatabaseConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub session_secret: String,
    pub session_ttl_secs: u64,
    pub allow_symlinks_outside_root: bool,
    pub allow_private_network_connections: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemConfig {
    pub default_local_root: PathBuf,
    pub temp_dir: Option<PathBuf>,
    pub show_hidden_default: bool,
    pub read_only_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitsConfig {
    pub max_upload_size: u64,
    pub max_editable_size: u64,
    pub max_preview_size: u64,
    pub max_directory_entries: usize,
    pub max_concurrent_transfers: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
            },
            security: SecurityConfig {
                session_secret: "dev_secret_change_in_production_32_chars_min".to_string(),
                session_ttl_secs: 86400 * 7, // 7 days
                allow_symlinks_outside_root: false,
                allow_private_network_connections: true, // dev default
            },
            filesystem: FilesystemConfig {
                default_local_root: PathBuf::from("./storage"),
                temp_dir: Some(PathBuf::from("./storage/temp")),
                show_hidden_default: false,
                read_only_default: false,
            },
            limits: LimitsConfig {
                max_upload_size: 1024 * 1024 * 1024,      // 1 GB
                max_editable_size: 10 * 1024 * 1024,      // 10 MB
                max_preview_size: 25 * 1024 * 1024,       // 25 MB
                max_directory_entries: 50_000,
                max_concurrent_transfers: 3,
            },
            database: DatabaseConfig {
                url: "sqlite://./filemanager.db?mode=rwc".to_string(),
            },
        }
    }
}

impl AppConfig {
    /// Load config from environment variables with fallback to defaults
    pub fn from_env_or_default() -> Self {
        let mut cfg = Self::default();

        if let Ok(host) = env::var("WFM_HOST").or_else(|_| env::var("HOST")) {
            cfg.server.host = host;
        }

        if let Ok(port_str) = env::var("WFM_PORT").or_else(|_| env::var("PORT")) {
            if let Ok(port) = port_str.parse::<u16>() {
                cfg.server.port = port;
            }
        }

        if let Ok(root) = env::var("WFM_ROOT_PATH").or_else(|_| env::var("WFM_LOCAL_ROOT")) {
            cfg.filesystem.default_local_root = PathBuf::from(root);
        }

        if let Ok(temp) = env::var("WFM_TEMP_DIR") {
            cfg.filesystem.temp_dir = Some(PathBuf::from(temp));
        }

        if let Ok(db_url) = env::var("WFM_DATABASE_URL").or_else(|_| env::var("DATABASE_URL")) {
            cfg.database.url = db_url;
        }

        if let Ok(symlinks) = env::var("WFM_ALLOW_SYMLINKS") {
            cfg.security.allow_symlinks_outside_root = symlinks == "1" || symlinks.to_lowercase() == "true";
        }

        if let Ok(secret) = env::var("WFM_SESSION_SECRET") {
            cfg.security.session_secret = secret;
        }

        if let Ok(max_upload_mb) = env::var("WFM_MAX_UPLOAD_MB") {
            if let Ok(mb) = max_upload_mb.parse::<u64>() {
                cfg.limits.max_upload_size = mb * 1024 * 1024;
            }
        }

        cfg
    }
}
