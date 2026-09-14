use std::{future::Future, time::Duration};
use tokio::time::{sleep, Instant};

pub const DEFAULT_EVENTUALLY_TIMEOUT: Duration = Duration::from_secs(5);
pub const DEFAULT_EVENTUALLY_POLL_INTERVAL: Duration = Duration::from_millis(25);

/// Probe until a value is produced or the named deadline expires.
///
/// Tests should use this for externally observable asynchronous convergence instead
/// of sleeping for an assumed amount of scheduler or I/O time and asserting once.
pub async fn eventually<T, F, Fut>(
    description: &str,
    timeout: Duration,
    poll_interval: Duration,
    mut probe: F,
) -> T
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Option<T>>,
{
    assert!(!timeout.is_zero(), "eventually timeout must be non-zero");
    assert!(
        !poll_interval.is_zero(),
        "eventually poll interval must be non-zero"
    );

    let deadline = Instant::now() + timeout;
    loop {
        if let Some(value) = probe().await {
            return value;
        }

        let now = Instant::now();
        if now >= deadline {
            panic!("timed out waiting for {description} after {timeout:?}");
        }

        sleep(poll_interval.min(deadline - now)).await;
    }
}

pub async fn eventually_default<T, F, Fut>(description: &str, probe: F) -> T
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Option<T>>,
{
    eventually(
        description,
        DEFAULT_EVENTUALLY_TIMEOUT,
        DEFAULT_EVENTUALLY_POLL_INTERVAL,
        probe,
    )
    .await
}
