use futures::FutureExt;
use std::collections::HashMap;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskCriticality {
    BestEffort,
    ReadinessCritical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestartPolicy {
    pub max_restarts: u32,
    pub backoff: Duration,
    /// A task that remains alive for at least this long gets a fresh restart budget.
    /// This prevents a few isolated failures spread over a long process lifetime from
    /// permanently exhausting the budget while still bounding tight crash loops.
    pub reset_after: Duration,
}

impl RestartPolicy {
    pub const NEVER: Self = Self {
        max_restarts: 0,
        backoff: Duration::ZERO,
        reset_after: Duration::ZERO,
    };

    pub const fn bounded(max_restarts: u32, backoff: Duration, reset_after: Duration) -> Self {
        Self {
            max_restarts,
            backoff,
            reset_after,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TaskHealthSnapshot {
    pub consecutive_failures: u32,
    pub last_error: Option<String>,
    pub last_success_age: Option<Duration>,
    pub criticality: TaskCriticality,
    pub restart_count: u32,
    pub restart_exhausted: bool,
}

#[derive(Debug, Clone)]
struct TaskHealth {
    consecutive_failures: u32,
    last_error: Option<String>,
    last_success: Option<Instant>,
    criticality: TaskCriticality,
    restart_count: u32,
    restart_exhausted: bool,
}

impl Default for TaskHealth {
    fn default() -> Self {
        Self {
            consecutive_failures: 0,
            last_error: None,
            last_success: None,
            criticality: TaskCriticality::BestEffort,
            restart_count: 0,
            restart_exhausted: false,
        }
    }
}

/// Supervised task runner tracking background asynchronous tasks across the runtime.
#[derive(Clone, Debug, Default)]
pub struct TaskSupervisor {
    tracker: TaskTracker,
    health: Arc<RwLock<HashMap<&'static str, TaskHealth>>>,
}

impl TaskSupervisor {
    pub fn new() -> Self {
        Self {
            tracker: TaskTracker::new(),
            health: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn register_task(&self, name: &'static str, criticality: TaskCriticality) {
        if let Ok(mut health) = self.health.write() {
            let entry = health.entry(name).or_default();
            entry.criticality = criticality;
        }
    }

    /// Spawns a named background task tracked by this supervisor.
    ///
    /// This compatibility path is intentionally best-effort and has no restart policy.
    /// Long-lived workers whose unexpected exit affects serving correctness should use
    /// `spawn_resilient` with an explicit criticality and bounded restart policy.
    pub fn spawn<F>(&self, name: &'static str, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.register_task(name, TaskCriticality::BestEffort);
        tracing::debug!(task.name = name, "supervisor.spawn");
        self.tracker.spawn(async move {
            let res = future.await;
            tracing::debug!(task.name = name, "supervisor.task_finished");
            res
        })
    }

    /// Runs a long-lived task under a bounded restart policy.
    ///
    /// The factory is invoked again after an unexpected `Err`, panic, or clean exit while
    /// shutdown has not been requested. Restarts are bounded and delayed. A worker that
    /// survives `reset_after` receives a fresh budget, preventing isolated failures over a
    /// long uptime from accumulating forever. Shutdown never consumes restart budget and
    /// never starts another worker instance.
    pub fn spawn_resilient<F, Fut, E>(
        &self,
        name: &'static str,
        criticality: TaskCriticality,
        policy: RestartPolicy,
        shutdown: CancellationToken,
        factory: F,
    ) -> JoinHandle<()>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), E>> + Send + 'static,
        E: ToString + Send + 'static,
    {
        self.register_task(name, criticality);
        let supervisor = self.clone();
        let factory = Arc::new(factory);

        tracing::debug!(task.name = name, ?criticality, ?policy, "supervisor.spawn_resilient");
        self.tracker.spawn(async move {
            let mut restarts_in_budget = 0u32;

            loop {
                if shutdown.is_cancelled() {
                    break;
                }

                let started_at = Instant::now();
                let outcome = AssertUnwindSafe((factory)()).catch_unwind().await;

                if shutdown.is_cancelled() {
                    break;
                }

                if policy.reset_after != Duration::ZERO && started_at.elapsed() >= policy.reset_after {
                    restarts_in_budget = 0;
                    supervisor.reset_restart_budget(name);
                }

                let error = match outcome {
                    Ok(Ok(())) => "task exited unexpectedly before shutdown".to_string(),
                    Ok(Err(error)) => error.to_string(),
                    Err(payload) => format!("task panicked: {}", panic_message(payload)),
                };
                supervisor.record_failure(name, &error);

                if restarts_in_budget >= policy.max_restarts {
                    supervisor.mark_restart_exhausted(name, &error);
                    tracing::error!(
                        task.name = name,
                        restarts = restarts_in_budget,
                        max_restarts = policy.max_restarts,
                        "supervisor.restart_exhausted"
                    );
                    break;
                }

                restarts_in_budget = restarts_in_budget.saturating_add(1);
                supervisor.record_restart(name, restarts_in_budget);
                tracing::warn!(
                    task.name = name,
                    restart = restarts_in_budget,
                    max_restarts = policy.max_restarts,
                    backoff_ms = policy.backoff.as_millis(),
                    %error,
                    "supervisor.restarting_task"
                );

                if policy.backoff != Duration::ZERO {
                    tokio::select! {
                        _ = shutdown.cancelled() => break,
                        _ = tokio::time::sleep(policy.backoff) => {}
                    }
                }
            }

            tracing::debug!(task.name = name, "supervisor.resilient_task_finished");
        })
    }

    /// Marks one successful maintenance iteration. Success resets the consecutive
    /// failure counter so readiness/metrics consumers can distinguish recovered
    /// workers from persistently degraded ones.
    pub fn record_success(&self, name: &'static str) {
        if let Ok(mut health) = self.health.write() {
            let entry = health.entry(name).or_default();
            entry.consecutive_failures = 0;
            entry.last_error = None;
            entry.last_success = Some(Instant::now());
        }
    }

    /// Marks one failed maintenance iteration and preserves the most recent error.
    pub fn record_failure(&self, name: &'static str, error: impl ToString) {
        let error = error.to_string();
        if let Ok(mut health) = self.health.write() {
            let entry = health.entry(name).or_default();
            entry.consecutive_failures = entry.consecutive_failures.saturating_add(1);
            entry.last_error = Some(error.clone());
        }
        tracing::error!(task.name = name, %error, "supervisor.background_iteration_failed");
    }

    fn record_restart(&self, name: &'static str, restart_count: u32) {
        if let Ok(mut health) = self.health.write() {
            let entry = health.entry(name).or_default();
            entry.restart_count = restart_count;
            entry.restart_exhausted = false;
        }
    }

    fn reset_restart_budget(&self, name: &'static str) {
        if let Ok(mut health) = self.health.write() {
            let entry = health.entry(name).or_default();
            entry.restart_count = 0;
            entry.restart_exhausted = false;
        }
    }

    fn mark_restart_exhausted(&self, name: &'static str, error: &str) {
        if let Ok(mut health) = self.health.write() {
            let entry = health.entry(name).or_default();
            entry.restart_exhausted = true;
            entry.last_error = Some(error.to_string());
        }
    }

    pub fn task_health(&self, name: &'static str) -> Option<TaskHealthSnapshot> {
        let health = self.health.read().ok()?;
        let entry = health.get(name)?;
        Some(snapshot(entry))
    }

    pub fn degraded_tasks(&self, failure_threshold: u32) -> Vec<(&'static str, TaskHealthSnapshot)> {
        let Ok(health) = self.health.read() else {
            return Vec::new();
        };
        health
            .iter()
            .filter(|(_, entry)| entry.consecutive_failures >= failure_threshold)
            .map(|(name, entry)| (*name, snapshot(entry)))
            .collect()
    }

    /// Returns only failures that should affect HTTP readiness. Best-effort maintenance
    /// failures remain observable through `task_health` / `degraded_tasks` but do not
    /// remove the server from service. Critical tasks degrade readiness after the normal
    /// iteration threshold or immediately once their bounded restart budget is exhausted.
    pub fn readiness_degraded_tasks(
        &self,
        failure_threshold: u32,
    ) -> Vec<(&'static str, TaskHealthSnapshot)> {
        let Ok(health) = self.health.read() else {
            return Vec::new();
        };
        health
            .iter()
            .filter(|(_, entry)| {
                entry.criticality == TaskCriticality::ReadinessCritical
                    && (entry.restart_exhausted
                        || entry.consecutive_failures >= failure_threshold)
            })
            .map(|(name, entry)| (*name, snapshot(entry)))
            .collect()
    }

    /// Closes the supervisor so no new tasks can be tracked, and waits for all active tasks
    /// to complete within the given timeout budget.
    pub async fn shutdown(&self, timeout: Duration) -> bool {
        self.tracker.close();
        tokio::select! {
            _ = self.tracker.wait() => {
                tracing::info!("supervisor.shutdown: all tasks drained cleanly");
                true
            }
            _ = tokio::time::sleep(timeout) => {
                tracing::warn!("supervisor.shutdown: timeout reached before all tasks drained");
                false
            }
        }
    }

    /// Returns the underlying TaskTracker reference.
    pub fn tracker(&self) -> &TaskTracker {
        &self.tracker
    }

    /// Returns the number of currently tracked active tasks.
    pub fn active_tasks(&self) -> usize {
        self.tracker.len()
    }
}

fn snapshot(entry: &TaskHealth) -> TaskHealthSnapshot {
    TaskHealthSnapshot {
        consecutive_failures: entry.consecutive_failures,
        last_error: entry.last_error.clone(),
        last_success_age: entry.last_success.map(|instant| instant.elapsed()),
        criticality: entry.criticality,
        restart_count: entry.restart_count,
        restart_exhausted: entry.restart_exhausted,
    }
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn background_health_tracks_failure_and_recovery() {
        let supervisor = TaskSupervisor::new();
        supervisor.record_failure("maintenance", "boom");
        supervisor.record_failure("maintenance", "boom again");

        let health = supervisor.task_health("maintenance").unwrap();
        assert_eq!(health.consecutive_failures, 2);
        assert_eq!(health.last_error.as_deref(), Some("boom again"));
        assert_eq!(supervisor.degraded_tasks(2).len(), 1);
        assert!(supervisor.readiness_degraded_tasks(2).is_empty());

        supervisor.record_success("maintenance");
        let health = supervisor.task_health("maintenance").unwrap();
        assert_eq!(health.consecutive_failures, 0);
        assert!(health.last_error.is_none());
        assert!(health.last_success_age.is_some());
        assert!(supervisor.degraded_tasks(1).is_empty());
    }

    #[tokio::test]
    async fn resilient_task_restarts_then_recovers_until_shutdown() {
        let supervisor = TaskSupervisor::new();
        let shutdown = CancellationToken::new();
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_for_task = attempts.clone();
        let shutdown_for_task = shutdown.clone();

        let handle = supervisor.spawn_resilient(
            "critical-worker",
            TaskCriticality::ReadinessCritical,
            RestartPolicy::bounded(3, Duration::from_millis(1), Duration::from_secs(60)),
            shutdown.clone(),
            move || {
                let attempts = attempts_for_task.clone();
                let shutdown = shutdown_for_task.clone();
                async move {
                    let attempt = attempts.fetch_add(1, Ordering::SeqCst);
                    if attempt == 0 {
                        return Err("transient failure");
                    }
                    shutdown.cancelled().await;
                    Ok::<(), &'static str>(())
                }
            },
        );

        tokio::time::timeout(Duration::from_secs(1), async {
            while attempts.load(Ordering::SeqCst) < 2 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("worker should restart");

        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        assert!(!supervisor.task_health("critical-worker").unwrap().restart_exhausted);
        shutdown.cancel();
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn critical_worker_exhaustion_degrades_readiness_health() {
        let supervisor = TaskSupervisor::new();
        let shutdown = CancellationToken::new();
        let handle = supervisor.spawn_resilient(
            "critical-worker",
            TaskCriticality::ReadinessCritical,
            RestartPolicy::bounded(1, Duration::from_millis(1), Duration::from_secs(60)),
            shutdown,
            || async { Err::<(), _>("always fails") },
        );

        handle.await.unwrap();
        let health = supervisor.task_health("critical-worker").unwrap();
        assert!(health.restart_exhausted);
        assert_eq!(health.restart_count, 1);
        assert_eq!(supervisor.readiness_degraded_tasks(99).len(), 1);
    }

    #[tokio::test]
    async fn shutdown_does_not_restart_resilient_worker() {
        let supervisor = TaskSupervisor::new();
        let shutdown = CancellationToken::new();
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_for_task = attempts.clone();
        let shutdown_for_task = shutdown.clone();

        let handle = supervisor.spawn_resilient(
            "shutdown-worker",
            TaskCriticality::ReadinessCritical,
            RestartPolicy::bounded(3, Duration::from_millis(1), Duration::from_secs(60)),
            shutdown.clone(),
            move || {
                let attempts = attempts_for_task.clone();
                let shutdown = shutdown_for_task.clone();
                async move {
                    attempts.fetch_add(1, Ordering::SeqCst);
                    shutdown.cancelled().await;
                    Ok::<(), &'static str>(())
                }
            },
        );

        tokio::task::yield_now().await;
        shutdown.cancel();
        handle.await.unwrap();
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
        assert!(!supervisor.task_health("shutdown-worker").unwrap().restart_exhausted);
    }
}
