use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::task::JoinHandle;
use tokio_util::task::TaskTracker;

#[derive(Debug, Clone)]
pub struct TaskHealthSnapshot {
    pub consecutive_failures: u32,
    pub last_error: Option<String>,
    pub last_success_age: Option<Duration>,
}

#[derive(Debug, Clone, Default)]
struct TaskHealth {
    consecutive_failures: u32,
    last_error: Option<String>,
    last_success: Option<Instant>,
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

    /// Spawns a named background task tracked by this supervisor.
    pub fn spawn<F>(&self, name: &'static str, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        tracing::debug!(task.name = name, "supervisor.spawn");
        self.tracker.spawn(async move {
            let res = future.await;
            tracing::debug!(task.name = name, "supervisor.task_finished");
            res
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

    pub fn task_health(&self, name: &'static str) -> Option<TaskHealthSnapshot> {
        let health = self.health.read().ok()?;
        let entry = health.get(name)?;
        Some(TaskHealthSnapshot {
            consecutive_failures: entry.consecutive_failures,
            last_error: entry.last_error.clone(),
            last_success_age: entry.last_success.map(|instant| instant.elapsed()),
        })
    }

    pub fn degraded_tasks(&self, failure_threshold: u32) -> Vec<(&'static str, TaskHealthSnapshot)> {
        let Ok(health) = self.health.read() else {
            return Vec::new();
        };
        health
            .iter()
            .filter(|(_, entry)| entry.consecutive_failures >= failure_threshold)
            .map(|(name, entry)| {
                (
                    *name,
                    TaskHealthSnapshot {
                        consecutive_failures: entry.consecutive_failures,
                        last_error: entry.last_error.clone(),
                        last_success_age: entry.last_success.map(|instant| instant.elapsed()),
                    },
                )
            })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_health_tracks_failure_and_recovery() {
        let supervisor = TaskSupervisor::new();
        supervisor.record_failure("maintenance", "boom");
        supervisor.record_failure("maintenance", "boom again");

        let health = supervisor.task_health("maintenance").unwrap();
        assert_eq!(health.consecutive_failures, 2);
        assert_eq!(health.last_error.as_deref(), Some("boom again"));
        assert_eq!(supervisor.degraded_tasks(2).len(), 1);

        supervisor.record_success("maintenance");
        let health = supervisor.task_health("maintenance").unwrap();
        assert_eq!(health.consecutive_failures, 0);
        assert!(health.last_error.is_none());
        assert!(health.last_success_age.is_some());
        assert!(supervisor.degraded_tasks(1).is_empty());
    }
}
