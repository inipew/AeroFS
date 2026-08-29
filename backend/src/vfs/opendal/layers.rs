use crate::config::ProviderStorageConfig;
use opendal::layers::{ConcurrentLimitLayer, LoggingLayer, RetryLayer, TimeoutLayer};
use opendal::Operator;
use std::time::Duration;

/// Apply standardized, deterministic OpenDAL layer pipeline based on provider storage configuration
/// Layer Order:
///   1. LoggingLayer (debug/trace visibility)
///   2. TimeoutLayer (independent control operation vs I/O transfer timeouts)
///   3. RetryLayer (transient socket, network, and temporary HTTP failure retries)
///   4. ConcurrentLimitLayer (storage provider concurrency budget)
pub fn apply_common_layers(op: Operator, config: &ProviderStorageConfig) -> Operator {
    let mut op = op.layer(LoggingLayer::default());

    // 1. Timeout Layer: independent control operation timeout and I/O timeout
    let timeout_layer = TimeoutLayer::new()
        .with_timeout(Duration::from_secs(config.control_timeout_secs.max(5)))
        .with_io_timeout(Duration::from_secs(config.io_timeout_secs.max(10)));
    op = op.layer(timeout_layer);

    // 2. Retry Layer for transient network/I/O retries with configured max attempts
    if config.retry_attempts > 0 {
        op = op.layer(
            RetryLayer::new()
                .with_max_times(config.retry_attempts)
                .with_jitter(),
        );
    }

    // 3. Concurrency Limit Layer (0 = unbounded / skip layer for local FS)
    if config.max_concurrency > 0 {
        op = op.layer(ConcurrentLimitLayer::new(config.max_concurrency));
    }

    op
}
