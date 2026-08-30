use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct StorageConfig {
    pub default_timeout_secs: u64,
    pub default_io_timeout_secs: u64,
    pub default_concurrency: usize,
    pub retry_attempts: usize,
    pub s3: ProviderStorageConfig,
    pub sftp: ProviderStorageConfig,
    pub ftp: ProviderStorageConfig,
    pub fs: ProviderStorageConfig,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            default_timeout_secs: 60,
            default_io_timeout_secs: 60,
            default_concurrency: 16,
            retry_attempts: 3,
            s3: ProviderStorageConfig {
                max_concurrency: 64,
                control_timeout_secs: 10,
                io_timeout_secs: 60,
                retry_attempts: 3,
            },
            sftp: ProviderStorageConfig {
                max_concurrency: 8,
                control_timeout_secs: 15,
                io_timeout_secs: 120,
                retry_attempts: 3,
            },
            ftp: ProviderStorageConfig {
                max_concurrency: 8,
                control_timeout_secs: 15,
                io_timeout_secs: 60,
                retry_attempts: 3,
            },
            fs: ProviderStorageConfig {
                max_concurrency: 0,
                control_timeout_secs: 10,
                io_timeout_secs: 60,
                retry_attempts: 1,
            },
        }
    }
}

impl StorageConfig {
    pub fn get_provider_config(&self, scheme: &str) -> ProviderStorageConfig {
        match scheme {
            "s3" => self.s3.clone(),
            "sftp" => self.sftp.clone(),
            "ftp" | "ftps" => self.ftp.clone(),
            "fs" => self.fs.clone(),
            _ => ProviderStorageConfig {
                max_concurrency: self.default_concurrency,
                control_timeout_secs: self.default_timeout_secs,
                io_timeout_secs: self.default_io_timeout_secs,
                retry_attempts: self.retry_attempts,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ProviderStorageConfig {
    pub max_concurrency: usize,
    pub control_timeout_secs: u64,
    pub io_timeout_secs: u64,
    pub retry_attempts: usize,
}

impl Default for ProviderStorageConfig {
    fn default() -> Self {
        Self {
            max_concurrency: 16,
            control_timeout_secs: 30,
            io_timeout_secs: 60,
            retry_attempts: 3,
        }
    }
}
