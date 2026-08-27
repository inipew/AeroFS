use crate::errors::VfsError;
use opendal::Operator;
use std::time::Duration;

/// Build an OpenDAL Operator for Local Filesystem (Fs service)
pub fn build_fs_operator(root: &str) -> Result<Operator, VfsError> {
    let mut builder = opendal::services::Fs::default();
    builder = builder.root(root);
    let op = Operator::new(builder)
        .map_err(|e| VfsError::ConnectionError(format!("Failed to init Local Fs Operator: {}", e)))?;
    Ok(op)
}

/// Build an OpenDAL Operator for Amazon S3 / S3-Compatible Storage
pub fn build_s3_operator(
    bucket: &str,
    region: Option<&str>,
    endpoint: Option<&str>,
    access_key_id: Option<&str>,
    secret_access_key: Option<&str>,
    root: Option<&str>,
) -> Result<Operator, VfsError> {
    let mut builder = opendal::services::S3::default();
    builder = builder.bucket(bucket);

    if let Some(r) = region {
        builder = builder.region(r);
    }
    if let Some(e) = endpoint {
        builder = builder.endpoint(e);
    }
    if let Some(a) = access_key_id {
        builder = builder.access_key_id(a);
    }
    if let Some(s) = secret_access_key {
        builder = builder.secret_access_key(s);
    }
    if let Some(rt) = root {
        builder = builder.root(rt);
    }

    let op = Operator::new(builder)
        .map_err(|e| VfsError::ConnectionError(format!("Failed to init S3 Operator: {}", e)))?;

    // Protect with TimeoutLayer (15 seconds)
    let op = op.layer(opendal::layers::TimeoutLayer::new().with_timeout(Duration::from_secs(15)));
    Ok(op)
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

    // Protect with TimeoutLayer (15 seconds) so unresponsive hosts fail gracefully
    let op = op.layer(opendal::layers::TimeoutLayer::new().with_timeout(Duration::from_secs(15)));
    Ok(op)
}

/// Build an OpenDAL Operator for SFTP / SSH
pub fn build_sftp_operator(
    host: &str,
    port: u16,
    user: Option<&str>,
    key: Option<&str>,
    root: Option<&str>,
) -> Result<Operator, VfsError> {
    let endpoint = format!("ssh://{}:{}", host, port);
    let mut builder = opendal::services::Sftp::default();
    builder = builder.endpoint(&endpoint);

    if let Some(u) = user {
        builder = builder.user(u);
    }
    if let Some(k) = key {
        builder = builder.key(k);
    }
    if let Some(rt) = root {
        builder = builder.root(rt);
    }

    let op = Operator::new(builder)
        .map_err(|e| VfsError::ConnectionError(format!("Failed to init SFTP Operator: {}", e)))?;

    // Protect with TimeoutLayer (15 seconds)
    let op = op.layer(opendal::layers::TimeoutLayer::new().with_timeout(Duration::from_secs(15)));
    Ok(op)
}
