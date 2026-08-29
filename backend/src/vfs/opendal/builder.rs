use super::layers::apply_common_layers;
use crate::config::ProviderStorageConfig;
use crate::domain::SftpAuth;
use crate::errors::VfsError;
use opendal::Operator;

/// Build an OpenDAL Operator for Local Filesystem (Fs service)
pub fn build_fs_operator(root: &str) -> Result<Operator, VfsError> {
    build_fs_operator_with_config(root, None)
}

pub fn build_fs_operator_with_config(
    root: &str,
    config: Option<&ProviderStorageConfig>,
) -> Result<Operator, VfsError> {
    let mut builder = opendal::services::Fs::default();
    builder = builder.root(root);
    let op = Operator::new(builder).map_err(|e| {
        VfsError::ConnectionError(format!("Failed to init Local Fs Operator: {}", e))
    })?;

    let default_cfg = ProviderStorageConfig {
        max_concurrency: 0,
        control_timeout_secs: 10,
        io_timeout_secs: 60,
        retry_attempts: 1,
    };
    let cfg = config.unwrap_or(&default_cfg);
    Ok(apply_common_layers(op, cfg))
}

/// Build an OpenDAL Operator for Amazon S3 / S3-Compatible Storage (AWS, MinIO, R2, Wasabi)
pub fn build_s3_operator(
    bucket: &str,
    region: Option<&str>,
    endpoint: Option<&str>,
    access_key_id: Option<&str>,
    secret_access_key: Option<&str>,
    root: Option<&str>,
) -> Result<Operator, VfsError> {
    build_s3_operator_with_config(
        bucket,
        region,
        endpoint,
        access_key_id,
        secret_access_key,
        root,
        None,
    )
}

pub fn build_s3_operator_with_config(
    bucket: &str,
    region: Option<&str>,
    endpoint: Option<&str>,
    access_key_id: Option<&str>,
    secret_access_key: Option<&str>,
    root: Option<&str>,
    config: Option<&ProviderStorageConfig>,
) -> Result<Operator, VfsError> {
    let mut builder = opendal::services::S3::default();
    builder = builder.bucket(bucket);

    let reg = region
        .filter(|r| !r.trim().is_empty())
        .unwrap_or("us-east-1");
    builder = builder.region(reg);

    if let Some(e) = endpoint {
        if !e.trim().is_empty() {
            builder = builder.endpoint(e);
        }
    }
    if let Some(a) = access_key_id {
        if !a.trim().is_empty() {
            builder = builder.access_key_id(a);
        }
    }
    if let Some(s) = secret_access_key {
        if !s.trim().is_empty() {
            builder = builder.secret_access_key(s);
        }
    }
    if let Some(rt) = root {
        builder = builder.root(rt);
    }

    let op = Operator::new(builder)
        .map_err(|e| VfsError::ConnectionError(format!("Failed to init S3 Operator: {}", e)))?;

    let default_cfg = ProviderStorageConfig {
        max_concurrency: 64,
        control_timeout_secs: 10,
        io_timeout_secs: 60,
        retry_attempts: 3,
    };
    let cfg = config.unwrap_or(&default_cfg);
    Ok(apply_common_layers(op, cfg))
}

/// Build an OpenDAL Operator for FTP / FTPS
pub fn build_ftp_operator(
    host: &str,
    port: u16,
    is_secure: bool,
    user: Option<&str>,
    password: Option<&str>,
    root: Option<&str>,
) -> Result<Operator, VfsError> {
    build_ftp_operator_with_config(host, port, is_secure, user, password, root, None)
}

pub fn build_ftp_operator_with_config(
    host: &str,
    port: u16,
    is_secure: bool,
    user: Option<&str>,
    password: Option<&str>,
    root: Option<&str>,
    config: Option<&ProviderStorageConfig>,
) -> Result<Operator, VfsError> {
    let scheme = if is_secure { "ftps" } else { "ftp" };
    let endpoint = format!("{}://{}:{}", scheme, host, port);
    let mut builder = opendal::services::Ftp::default();
    builder = builder.endpoint(&endpoint);

    if let Some(u) = user {
        builder = builder.user(u);
    }
    if let Some(p) = password {
        builder = builder.password(p);
    }
    if let Some(rt) = root {
        builder = builder.root(rt);
    }

    let op = Operator::new(builder)
        .map_err(|e| VfsError::ConnectionError(format!("Failed to init FTP Operator: {}", e)))?;

    let default_cfg = ProviderStorageConfig {
        max_concurrency: 8,
        control_timeout_secs: 15,
        io_timeout_secs: 60,
        retry_attempts: 3,
    };
    let cfg = config.unwrap_or(&default_cfg);
    Ok(apply_common_layers(op, cfg))
}

/// Build an OpenDAL Operator for SFTP / SSH with typed SftpAuth (Password / PrivateKey)
pub fn build_sftp_operator(
    host: &str,
    port: u16,
    user: Option<&str>,
    auth: Option<&SftpAuth>,
    root: Option<&str>,
) -> Result<Operator, VfsError> {
    build_sftp_operator_with_config(host, port, user, auth, root, None)
}

pub fn build_sftp_operator_with_config(
    host: &str,
    port: u16,
    user: Option<&str>,
    auth: Option<&SftpAuth>,
    root: Option<&str>,
    config: Option<&ProviderStorageConfig>,
) -> Result<Operator, VfsError> {
    let endpoint = format!("ssh://{}:{}", host, port);
    let mut builder = opendal::services::Sftp::default();
    builder = builder.endpoint(&endpoint);

    if let Some(u) = user {
        builder = builder.user(u);
    }

    // SFTP authentication handling (Key path or agent)
    if let Some(auth_method) = auth {
        match auth_method {
            SftpAuth::Password { password: _ } => {
                return Err(VfsError::NotSupported(
                    "SFTP password authentication is not natively supported by OpenDAL; please configure an SSH private key path or SSH agent.".into(),
                ));
            }
            SftpAuth::PrivateKey { key, passphrase: _ } => {
                if !key.trim().is_empty() {
                    builder = builder.key(key);
                }
            }
        }
    }

    if let Some(rt) = root {
        builder = builder.root(rt);
    }

    let op = Operator::new(builder)
        .map_err(|e| VfsError::ConnectionError(format!("Failed to init SFTP Operator: {}", e)))?;

    let default_cfg = ProviderStorageConfig {
        max_concurrency: 8,
        control_timeout_secs: 15,
        io_timeout_secs: 120,
        retry_attempts: 3,
    };
    let cfg = config.unwrap_or(&default_cfg);
    Ok(apply_common_layers(op, cfg))
}
