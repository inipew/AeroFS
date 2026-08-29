use std::future::Future;
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio_util::task::TaskTracker;

/// Supervised task runner tracking background asynchronous tasks across the runtime.
#[derive(Clone, Debug, Default)]
pub struct TaskSupervisor {
    tracker: TaskTracker,
}

impl TaskSupervisor {
    pub fn new() -> Self {
        Self {
            tracker: TaskTracker::new(),
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
