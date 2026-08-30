use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SecurityConfig {
    pub session_secret: String,
    pub session_ttl_secs: u64,
    pub allow_symlinks_outside_root: bool,
    pub allow_private_network_connections: bool,
    pub allowed_origins: Vec<String>,
    pub cookie_secure: bool,
    #[serde(default)]
    pub trusted_proxies: Vec<String>,
    #[serde(default)]
    pub credential_encryption_key: Option<String>,
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
            trusted_proxies: Vec::new(),
            credential_encryption_key: None,
        }
    }
}

impl SecurityConfig {
    pub fn session_ttl(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.session_ttl_secs)
    }
}
