use backend::cli::daemon_lock::{DaemonLock, ProcessStatus};
use std::fs;

#[test]
fn daemon_lock_is_exclusive_and_can_be_reacquired_after_release() {
    let temp = tempfile::tempdir().unwrap();
    let lock_path = temp.path().join("aerofs.lock");

    let first = DaemonLock::acquire(&lock_path).expect("first daemon lock should succeed");
    assert!(
        DaemonLock::acquire(&lock_path).is_err(),
        "second daemon lock must fail while the first owner is alive"
    );

    first.release();

    let second = DaemonLock::acquire(&lock_path)
        .expect("daemon lock should become available after explicit release");
    second.release();
}

#[test]
fn daemon_status_tracks_lock_acquire_and_release() {
    let temp = tempfile::tempdir().unwrap();
    let lock_path = temp.path().join("aerofs-status.lock");

    assert_eq!(
        DaemonLock::inspect_status(&lock_path, "127.0.0.1", 8080),
        ProcessStatus::Stopped
    );

    let lock = DaemonLock::acquire(&lock_path).unwrap();
    match DaemonLock::inspect_status(&lock_path, "127.0.0.1", 8080) {
        ProcessStatus::Running { pid, .. } => assert_eq!(pid, std::process::id()),
        other => panic!("expected running daemon status, got {other:?}"),
    }

    lock.release();
    assert_eq!(
        DaemonLock::inspect_status(&lock_path, "127.0.0.1", 8080),
        ProcessStatus::Stopped
    );
}

#[tokio::test]
async fn cli_dispatches_config_version_database_and_user_commands() {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("cli_test.db");
    let config_path = temp.path().join("config.toml");
    fs::write(
        &config_path,
        format!(
            r#"
[server]
host = "127.0.0.1"
port = 8888

[filesystem]
default_local_root = "{}"

[database]
url = "sqlite://{}?mode=rwc"
"#,
            temp.path().join("storage").display(),
            db_path.display()
        ),
    )
    .unwrap();

    let cli = |command| backend::cli::Cli {
        config: Some(config_path.clone()),
        json: true,
        quiet: false,
        verbose: false,
        log_level: None,
        command: Some(command),
    };

    backend::cli::run_cli(cli(backend::cli::Commands::Config(
        backend::cli::args::ConfigCommand {
            action: backend::cli::args::ConfigAction::Validate,
        },
    )))
    .await
    .unwrap();

    backend::cli::run_cli(cli(backend::cli::Commands::Version))
        .await
        .unwrap();

    backend::cli::run_cli(cli(backend::cli::Commands::Db(
        backend::cli::args::DbCommand {
            action: backend::cli::args::DbAction::Migrate,
        },
    )))
    .await
    .unwrap();

    backend::cli::run_cli(cli(backend::cli::Commands::Db(
        backend::cli::args::DbCommand {
            action: backend::cli::args::DbAction::Integrity,
        },
    )))
    .await
    .unwrap();

    backend::cli::run_cli(cli(backend::cli::Commands::User(
        backend::cli::args::UserCommand {
            action: backend::cli::args::UserAction::List,
        },
    )))
    .await
    .unwrap();
}
