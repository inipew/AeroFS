use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct BackgroundTaskDegradation {
    pub name: String,
    pub consecutive_failures: u32,
    pub last_error: Option<String>,
    pub restart_exhausted: bool,
    pub restart_count: u32,
}

#[derive(Debug, Clone)]
pub struct ReadinessSignals {
    pub runtime_running: bool,
    pub phase: &'static str,
    pub database_ok: bool,
    pub storage_ok: bool,
    pub active_providers: usize,
    pub degraded_tasks: Vec<BackgroundTaskDegradation>,
}

#[async_trait]
pub trait ReadinessProbe: Send + Sync {
    async fn signals(&self, background_failure_threshold: u32) -> ReadinessSignals;
}
