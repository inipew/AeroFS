use std::fs;
use std::path::PathBuf;

fn source(path: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(root.join(path)).expect("source file should be readable")
}

#[test]
fn app_state_only_exposes_runtime_view_capability() {
    let state = source("src/state.rs");
    assert!(state.contains("pub struct RuntimeOwner"));
    assert!(state.contains("pub struct RuntimeView"));
    assert!(state.contains("pub struct RuntimeState"));
    assert!(state.contains("pub(crate) runtime: RuntimeState"));
    assert!(!state.contains("pub struct AppRuntime"));
    assert!(!state.contains("pub runtime: RuntimeOwner"));
}

#[test]
fn bootstrap_returns_runtime_owner_separately() {
    let bootstrap = source("src/bootstrap.rs");
    assert!(bootstrap.contains("pub struct BuiltApplication"));
    assert!(bootstrap.contains("pub state: AppState"));
    assert!(bootstrap.contains("pub runtime: RuntimeOwner"));
    assert!(bootstrap.contains("pub async fn build_application"));
    assert!(bootstrap.contains("RuntimeState::new(runtime.view())"));
    assert!(bootstrap.contains("BuiltApplication { state, runtime }"));
}

#[test]
fn runtime_owner_cannot_be_silently_discarded_by_compatibility_builders() {
    let state = source("src/state.rs");
    let bootstrap = source("src/bootstrap.rs");

    assert!(!state.contains("pub async fn new_with_db"));
    assert!(!state.contains("build_app_state(config, db).await"));
    assert!(!bootstrap.contains("pub async fn build_app_state"));
    assert!(!bootstrap.contains("build_application(config, db).await.state"));
}

#[test]
fn cli_state_retains_and_cancels_runtime_owner() {
    let context = source("src/cli/context.rs");
    assert!(context.contains("pub struct CliState"));
    assert!(context.contains("runtime: RuntimeOwner"));
    assert!(context.contains("impl Deref for CliState"));
    assert!(context.contains("impl Drop for CliState"));
    assert!(context.contains("request_shutdown(ShutdownReason::Manual)"));
    assert!(context.contains("let built = build_application(self.config.clone(), pool).await"));
    assert!(!context.contains("AppState::new_with_db"));
}

#[test]
fn runtime_tasks_are_owned_outside_app_state() {
    let bootstrap = source("src/bootstrap.rs");
    assert!(bootstrap.contains("runtime: &RuntimeOwner"));
    assert!(!bootstrap.contains("fn spawn_runtime_tasks(state: &AppState)"));
    assert!(!bootstrap.contains("state.runtime.shutdown_token"));
    assert!(!bootstrap.contains("state.runtime.supervisor"));
}

#[test]
fn production_server_retains_runtime_owner() {
    let serve = source("src/cli/commands/serve.rs");
    assert!(serve.contains("build_application(config, db).await"));
    assert!(serve.contains("let runtime = built.runtime"));
    assert!(serve.contains("shutdown_signal(runtime: RuntimeOwner)"));
    assert!(!serve.contains("state.runtime"));
}

#[test]
fn runtime_owner_handles_do_not_leak_to_router() {
    let router = source("src/router.rs");
    for forbidden in [
        "RuntimeOwner",
        "force_shutdown_token",
        "task_tracker",
        ".supervisor",
    ] {
        assert!(
            !router.contains(forbidden),
            "router leaked process runtime ownership handle: {forbidden}"
        );
    }
}
