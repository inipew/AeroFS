use crate::domain::{Connection, ProviderKind, SftpAuth};
use crate::vfs::opendal::{
    build_fs_operator, build_ftp_operator, build_s3_operator, build_sftp_operator,
    OpenDalFileSystem,
};
use crate::vfs::FileSystem;
use std::path::PathBuf;
use std::sync::Arc;

pub struct ProviderFactory;

impl ProviderFactory {
    pub fn build_local(id: &str, root_path: PathBuf) -> anyhow::Result<Arc<dyn FileSystem>> {
        let root_str = root_path.to_string_lossy().to_string();
        let op = build_fs_operator(&root_str)?;
        Ok(Arc::new(OpenDalFileSystem::new_local(id, op, root_path)))
    }

    pub fn build(
        connection: &Connection,
        secret: Option<&str>,
    ) -> anyhow::Result<Arc<dyn FileSystem>> {
        match connection.provider {
            ProviderKind::Local => {
                Self::build_local(&connection.id, PathBuf::from(&connection.base_path))
            }
            ProviderKind::Ftp => {
                let host = connection.host.as_deref().unwrap_or("127.0.0.1");
                let port = connection.port.unwrap_or(21);
                let op = build_ftp_operator(
                    host,
                    port,
                    false,
                    connection.username.as_deref(),
                    secret,
                    Some(&connection.base_path),
                )?;
                Ok(Arc::new(OpenDalFileSystem::new(&connection.id, op)))
            }
            ProviderKind::Ftps => {
                let host = connection.host.as_deref().unwrap_or("127.0.0.1");
                let port = connection.port.unwrap_or(990);
                let op = build_ftp_operator(
                    host,
                    port,
                    true,
                    connection.username.as_deref(),
                    secret,
                    Some(&connection.base_path),
                )?;
                Ok(Arc::new(OpenDalFileSystem::new(&connection.id, op)))
            }
            ProviderKind::Sftp => {
                let host = connection.host.as_deref().unwrap_or("127.0.0.1");
                let port = connection.port.unwrap_or(22);
                let auth = secret.map(|s| SftpAuth::Password {
                    password: s.to_string(),
                });
                let op = build_sftp_operator(
                    host,
                    port,
                    connection.username.as_deref(),
                    auth.as_ref(),
                    Some(&connection.base_path),
                )?;
                Ok(Arc::new(OpenDalFileSystem::new(&connection.id, op)))
            }
            ProviderKind::S3 => {
                let bucket = connection.host.as_deref().unwrap_or("default-bucket");
                let op = build_s3_operator(
                    bucket,
                    None,
                    None,
                    connection.username.as_deref(),
                    secret,
                    Some(&connection.base_path),
                )?;
                Ok(Arc::new(OpenDalFileSystem::new(&connection.id, op)))
            }
        }
    }
}
