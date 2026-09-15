use backend::{
    runtime::{ResourceBudget, ResourceClass, TaskSupervisor},
    state::{RuntimeOwner, RuntimePhase, ShutdownReason},
    vfs::ProviderState,
};
use chrono::Utc;
use std::{sync::Arc, time::Duration};
use tokio::sync::{oneshot, Notify};

#[tokio::test]
async fn runtime_owner_tracks_phase_and_preserves_the_first_shutdown_reason() {
    let runtime = RuntimeOwner::default();
    let view = runtime.view();

    assert_eq!(runtime.phase(), RuntimePhase::Starting);
    assert_eq!(view.phase(), RuntimePhase::Starting);

    runtime.set_phase(RuntimePhase::Binding);
    assert_eq!(runtime.phase().as_str(), "binding");
    assert_eq!(view.phase(), RuntimePhase::Binding);

    runtime.set_phase(RuntimePhase::Running);
    assert!(view.is_running());
    assert!(!runtime.is_shutting_down());

    assert!(runtime.request_shutdown(ShutdownReason::CtrlC));
    assert_eq!(runtime.phase(), RuntimePhase::ShuttingDown);
    assert!(view.is_shutting_down());
    assert!(runtime.shutdown_token.is_cancelled());
    assert_eq!(runtime.shutdown_reason(), Some(ShutdownReason::CtrlC));

    assert!(!runtime.request_shutdown(ShutdownReason::Sigterm));
    assert!(!runtime.request_shutdown(ShutdownReason::Internal));
    assert_eq!(runtime.shutdown_reason(), Some(ShutdownReason::CtrlC));
}

#[tokio::test]
async fn runtime_owned_task_observes_shutdown_and_drains_without_scheduler_sleep() {
    let runtime = RuntimeOwner::default();
    let (started_tx, started_rx) = oneshot::channel();
    let (cleaned_tx, cleaned_rx) = oneshot::channel();
    let shutdown = runtime.shutdown_token.clone();

    runtime.task_tracker.spawn(async move {
        let _ = started_tx.send(());
        shutdown.cancelled().await;
        let _ = cleaned_tx.send(());
    });

    tokio::time::timeout(Duration::from_secs(2), started_rx)
        .await
        .expect("tracked task should start before deadline")
        .expect("tracked task dropped start signal");

    assert!(runtime.request_shutdown(ShutdownReason::Manual));
    runtime.task_tracker.close();

    tokio::time::timeout(Duration::from_secs(2), runtime.task_tracker.wait())
        .await
        .expect("tracked runtime task should drain before deadline");
    tokio::time::timeout(Duration::from_secs(2), cleaned_rx)
        .await
        .expect("tracked task should execute shutdown cleanup")
        .expect("tracked task dropped cleanup signal");
}

#[tokio::test]
async fn task_supervisor_drains_released_work_and_returns_to_zero_active_tasks() {
    let supervisor = TaskSupervisor::new();
    let release = Arc::new(Notify::new());
    let release_task = Arc::clone(&release);
    let (started_tx, started_rx) = oneshot::channel();

    supervisor.spawn("runtime-lifecycle-test", async move {
        let _ = started_tx.send(());
        release_task.notified().await;
    });

    tokio::time::timeout(Duration::from_secs(2), started_rx)
        .await
        .expect("supervised task should start before deadline")
        .expect("supervised task dropped start signal");
    assert_eq!(supervisor.active_tasks(), 1);

    release.notify_one();
    assert!(supervisor.shutdown(Duration::from_secs(2)).await);
    assert_eq!(supervisor.active_tasks(), 0);
}

#[tokio::test]
async fn resource_budget_capacity_is_returned_when_permits_drop() {
    let budget = ResourceBudget::new(4, 2, 2, 1, 1);
    let first = budget
        .acquire(ResourceClass::LocalIo)
        .await
        .expect("first local permit");
    let second = budget
        .acquire(ResourceClass::LocalIo)
        .await
        .expect("second local permit");
    assert_eq!(budget.available_local(), 0);

    drop(first);
    assert_eq!(budget.available_local(), 1);
    drop(second);
    assert_eq!(budget.available_local(), 2);
}

#[test]
fn provider_lifecycle_states_expose_stable_operational_semantics() {
    let initializing = ProviderState::Initializing;
    assert_eq!(initializing.as_str(), "initializing");
    assert!(!initializing.is_ready());
    assert!(!initializing.is_operational());

    let ready = ProviderState::Ready;
    assert_eq!(ready.as_str(), "ready");
    assert!(ready.is_ready());
    assert!(ready.is_operational());

    let degraded = ProviderState::Degraded {
        since: Utc::now(),
        reason: "rate limited".to_string(),
    };
    assert_eq!(degraded.as_str(), "degraded");
    assert!(!degraded.is_ready());
    assert!(degraded.is_operational());

    let draining = ProviderState::Draining;
    assert_eq!(draining.as_str(), "draining");
    assert!(!draining.is_ready());
    assert!(!draining.is_operational());
}
