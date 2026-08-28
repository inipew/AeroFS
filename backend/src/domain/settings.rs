use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(default)]
pub struct GeneralSettings {
    pub language: String,
    pub theme: String,
    pub default_view: String,   // "grid" | "list"
    pub default_sort: String,   // "name" | "size" | "modified"
    pub sort_direction: String, // "asc" | "desc"
    pub show_hidden_default: bool,
    pub confirm_destructive: bool,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            language: "en".to_string(),
            theme: "dark".to_string(),
            default_view: "grid".to_string(),
            default_sort: "name".to_string(),
            sort_direction: "asc".to_string(),
            show_hidden_default: false,
            confirm_destructive: true,
        }
    }
}

fn default_max_editable_size() -> u64 {
    10 * 1024 * 1024
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(default)]
pub struct FileManagerSettings {
    pub default_layout: String, // "single" | "split"
    pub show_breadcrumbs: bool,
    pub show_file_size: bool,
    pub show_permissions: bool,
    pub remember_last_directories: bool,
    #[serde(default = "default_max_editable_size")]
    pub max_editable_size: u64,
}

impl Default for FileManagerSettings {
    fn default() -> Self {
        Self {
            default_layout: "split".to_string(),
            show_breadcrumbs: true,
            show_file_size: true,
            show_permissions: true,
            remember_last_directories: true,
            max_editable_size: default_max_editable_size(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(default)]
pub struct TransferSettings {
    pub max_concurrent_transfers: usize,
    pub retry_attempts: usize,
    pub auto_retry: bool,
    pub show_notifications: bool,
}

impl Default for TransferSettings {
    fn default() -> Self {
        Self {
            max_concurrent_transfers: 4,
            retry_attempts: 3,
            auto_retry: true,
            show_notifications: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(default)]
pub struct ConnectionSettings {
    pub connection_timeout_secs: u64,
    pub health_check_interval_secs: u64,
    pub auto_reconnect: bool,
    pub default_local_root: String,
    pub temp_dir: String,
}

impl Default for ConnectionSettings {
    fn default() -> Self {
        Self {
            connection_timeout_secs: 60,
            health_check_interval_secs: 30,
            auto_reconnect: true,
            default_local_root: "./storage".to_string(),
            temp_dir: "./storage/temp".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(default)]
pub struct SecuritySettings {
    pub allow_symlinks_outside_root: bool,
    pub confirm_permanent_delete: bool,
    pub read_only_default: bool,
    pub session_timeout_secs: u64,
}

impl Default for SecuritySettings {
    fn default() -> Self {
        Self {
            allow_symlinks_outside_root: false,
            confirm_permanent_delete: true,
            read_only_default: false,
            session_timeout_secs: 86400,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(default)]
pub struct AdvancedSettings {
    pub log_level: String, // "info" | "debug" | "trace"
    pub enable_telemetry: bool,
    pub enable_tracing: bool,
    pub directory_cache_ttl_secs: u64,
}

impl Default for AdvancedSettings {
    fn default() -> Self {
        Self {
            log_level: "info".to_string(),
            enable_telemetry: true,
            enable_tracing: true,
            directory_cache_ttl_secs: 0,
        }
    }
}

fn default_language() -> String {
    "en".to_string()
}
fn default_theme() -> String {
    "dark".to_string()
}
fn default_view() -> String {
    "grid".to_string()
}
fn default_density() -> String {
    "comfortable".to_string()
}
fn default_sort() -> String {
    "name".to_string()
}
fn default_sort_direction() -> String {
    "asc".to_string()
}
fn default_layout() -> String {
    "split".to_string()
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(default)]
pub struct UserPreferences {
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_view")]
    pub default_view: String, // "grid" | "list"
    #[serde(default = "default_density")]
    pub list_density: String, // "comfortable" | "compact" | "dense"
    #[serde(default = "default_sort", alias = "sort_field")]
    pub default_sort: String, // "name" | "size" | "modified"
    #[serde(default = "default_sort_direction", alias = "sort_order")]
    pub sort_direction: String, // "asc" | "desc"
    #[serde(default)]
    pub show_hidden: bool,
    #[serde(default = "default_true")]
    pub confirm_destructive: bool,
    #[serde(default = "default_layout")]
    pub default_layout: String, // "single" | "split"
    #[serde(default = "default_true")]
    pub show_breadcrumbs: bool,
    #[serde(default = "default_true")]
    pub show_file_size: bool,
    #[serde(default = "default_true")]
    pub show_permissions: bool,
    #[serde(default = "default_true", alias = "remember_last_dir")]
    pub remember_last_directories: bool,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            language: "en".to_string(),
            theme: "dark".to_string(),
            default_view: "grid".to_string(),
            list_density: "comfortable".to_string(),
            default_sort: "name".to_string(),
            sort_direction: "asc".to_string(),
            show_hidden: false,
            confirm_destructive: true,
            default_layout: "split".to_string(),
            show_breadcrumbs: true,
            show_file_size: true,
            show_permissions: true,
            remember_last_directories: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(default)]
pub struct SystemSettings {
    pub default_local_root: String,
    pub temp_dir: String,
    pub read_only_default: bool,
    pub max_concurrent_transfers: usize,
    pub retry_attempts: usize,
    pub connection_timeout_secs: u64,
    pub allow_symlinks_outside_root: bool,
}

impl Default for SystemSettings {
    fn default() -> Self {
        Self {
            default_local_root: "./storage".to_string(),
            temp_dir: "./storage/temp".to_string(),
            read_only_default: false,
            max_concurrent_transfers: 4,
            retry_attempts: 3,
            connection_timeout_secs: 60,
            allow_symlinks_outside_root: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
#[serde(default)]
pub struct AppSettings {
    #[serde(default)]
    pub general: GeneralSettings,
    #[serde(default)]
    pub file_manager: FileManagerSettings,
    #[serde(default)]
    pub transfers: TransferSettings,
    #[serde(default)]
    pub connections: ConnectionSettings,
    #[serde(default)]
    pub security: SecuritySettings,
    #[serde(default)]
    pub advanced: AdvancedSettings,
}
