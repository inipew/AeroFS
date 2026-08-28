use crate::domain::SftpAuth;
use crate::errors::VfsError;
use opendal::layers::{ConcurrentLimitLayer, LoggingLayer, RetryLayer, TimeoutLayer};
use opendal::Operator;
use std::time::Duration;

/// Build an OpenDAL Operator for Local Filesystem (Fs service)
pub fn build_fs_operator(root: &str) -> Result<Operator, VfsError> {
    let mut builder = opendal::services::Fs::default();
    builder = builder.root(root);
    let op = Operator::new(builder).map_err(|e| {
        VfsError::ConnectionError(format!("Failed to init Local Fs Operator: {}", e))
    })?;
    let op = op.layer(LoggingLayer::default());
    Ok(op)
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

    // Attach Resiliency & Observability Layers: Retry, Concurrent Limit (16), Timeout (60s), Logging
    let op = op
        .layer(LoggingLayer::default())
        .layer(RetryLayer::new())
        .layer(ConcurrentLimitLayer::new(16))
        .layer(TimeoutLayer::new().with_timeout(Duration::from_secs(60)));

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

    // Attach Resiliency & Observability Layers
    let op = op
        .layer(LoggingLayer::default())
        .layer(RetryLayer::new())
        .layer(ConcurrentLimitLayer::new(16))
        .layer(TimeoutLayer::new().with_timeout(Duration::from_secs(60)));

    Ok(op)
}

/// Build an OpenDAL Operator for SFTP / SSH with typed SftpAuth (Password / PrivateKey)
pub fn build_sftp_operator(
    host: &str,
    port: u16,
    user: Option<&str>,
    auth: Option<&SftpAuth>,
    root: Option<&str>,
) -> Result<Operator, VfsError> {
    let endpoint = format!("ssh://{}:{}", host, port);
    let mut builder = opendal::services::Sftp::default();
    builder = builder.endpoint(&endpoint);

    if let Some(u) = user {
        builder = builder.user(u);
    }

    if let Some(auth_method) = auth {
        match auth_method {
            SftpAuth::Password { password: _ } => {
                // SFTP password handled by agent/ssh config
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

    // Attach Resiliency & Observability Layers
    let op = op
        .layer(LoggingLayer::default())
        .layer(RetryLayer::new())
        .layer(ConcurrentLimitLayer::new(16))
        .layer(TimeoutLayer::new().with_timeout(Duration::from_secs(60)));

    Ok(op)
}
