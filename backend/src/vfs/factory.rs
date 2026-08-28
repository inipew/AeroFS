use crate::config::ProviderStorageConfig;
use crate::domain::{Connection, ProviderKind, SftpAuth};
use crate::vfs::opendal::{
    build_fs_operator_with_config, build_ftp_operator_with_config, build_s3_operator_with_config,
    build_sftp_operator_with_config, OpenDalFileSystem,
};
use crate::vfs::FileSystem;
use std::path::PathBuf;
use std::sync::Arc;

pub struct ProviderFactory;

impl ProviderFactory {
    pub fn build_local(id: &str, root_path: PathBuf) -> anyhow::Result<Arc<dyn FileSystem>> {
        Self::build_local_with_config(id, root_path, None)
    }

    pub fn build_local_with_config(
        id: &str,
        root_path: PathBuf,
        config: Option<&ProviderStorageConfig>,
    ) -> anyhow::Result<Arc<dyn FileSystem>> {
        let root_str = root_path.to_string_lossy().to_string();
        let op = build_fs_operator_with_config(&root_str, config)?;
        Ok(Arc::new(OpenDalFileSystem::new_local(id, op, root_path)))
    }

    pub fn build(
        connection: &Connection,
        secret: Option<&str>,
    ) -> anyhow::Result<Arc<dyn FileSystem>> {
        Self::build_with_config(connection, secret, None)
    }

    pub fn build_with_config(
        connection: &Connection,
        secret: Option<&str>,
        config: Option<&ProviderStorageConfig>,
    ) -> anyhow::Result<Arc<dyn FileSystem>> {
        match connection.provider {
            ProviderKind::Local => Self::build_local_with_config(
                &connection.id,
                PathBuf::from(&connection.base_path),
                config,
            ),
            ProviderKind::Ftp => {
                let host = connection.host.as_deref().unwrap_or("127.0.0.1");
                let port = connection.port.unwrap_or(21);
                let op = build_ftp_operator_with_config(
                    host,
                    port,
                    false,
                    connection.username.as_deref(),
                    secret,
                    Some(&connection.base_path),
                    config,
                )?;
                Ok(Arc::new(OpenDalFileSystem::new(&connection.id, op)))
            }
            ProviderKind::Ftps => {
                let host = connection.host.as_deref().unwrap_or("127.0.0.1");
                let port = connection.port.unwrap_or(990);
                let op = build_ftp_operator_with_config(
                    host,
                    port,
                    true,
                    connection.username.as_deref(),
                    secret,
                    Some(&connection.base_path),
                    config,
                )?;
                Ok(Arc::new(OpenDalFileSystem::new(&connection.id, op)))
            }
            ProviderKind::Sftp => {
                let host = connection.host.as_deref().unwrap_or("127.0.0.1");
                let port = connection.port.unwrap_or(22);
                let auth = secret.map(|s| SftpAuth::Password {
                    password: s.to_string(),
                });
                let op = build_sftp_operator_with_config(
                    host,
                    port,
                    connection.username.as_deref(),
                    auth.as_ref(),
                    Some(&connection.base_path),
                    config,
                )?;
                Ok(Arc::new(OpenDalFileSystem::new(&connection.id, op)))
            }
            ProviderKind::S3 => {
                let bucket = connection.host.as_deref().unwrap_or("default-bucket");
                let op = build_s3_operator_with_config(
                    bucket,
                    None,
                    None,
                    connection.username.as_deref(),
                    secret,
                    Some(&connection.base_path),
                    config,
                )?;
                Ok(Arc::new(OpenDalFileSystem::new(&connection.id, op)))
            }
        }
    }
}
