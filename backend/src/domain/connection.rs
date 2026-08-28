use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Local,
    Ftp,
    Ftps,
    Sftp,
    S3,
}

impl ProviderKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProviderKind::Local => "local",
            ProviderKind::Ftp => "ftp",
            ProviderKind::Ftps => "ftps",
            ProviderKind::Sftp => "sftp",
            ProviderKind::S3 => "s3",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SftpAuth {
    Password {
        password: String,
    },
    PrivateKey {
        key: String,
        passphrase: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "provider", rename_all = "lowercase")]
pub enum ProviderConfig {
    Local {
        root: String,
    },
    Ftp {
        host: String,
        port: u16,
        is_secure: bool,
        username: Option<String>,
        password: Option<String>,
        root: Option<String>,
    },
    Sftp {
        host: String,
        port: u16,
        username: Option<String>,
        auth: Option<SftpAuth>,
        root: Option<String>,
    },
    S3 {
        bucket: String,
        region: Option<String>,
        endpoint: Option<String>,
        access_key_id: Option<String>,
        secret_access_key: Option<String>,
        root: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Connection {
    pub id: String,
    pub name: String,
    pub provider: ProviderKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    pub base_path: String,
    pub read_only: bool,
    pub enabled: bool,
    pub status: ConnectionStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Connection {
    pub fn new_local(name: impl Into<String>, base_path: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: format!("conn_local_{}", &Uuid::new_v4().to_string()[..8]),
            name: name.into(),
            provider: ProviderKind::Local,
            host: None,
            port: None,
            username: None,
            base_path: base_path.into(),
            read_only: false,
            enabled: true,
            status: ConnectionStatus::Connected,
            error_message: None,
            created_at: now,
            updated_at: now,
        }
    }
}
