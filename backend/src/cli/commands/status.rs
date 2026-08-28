use crate::cli::context::CliContext;
use crate::cli::daemon_lock::{DaemonLock, ProcessStatus};
use crate::cli::error::{CliError, ExitCode};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Serialize)]
struct StatusOutput {
    status: ProcessStatus,
    configured_host: String,
    configured_port: u16,
    storage_root: String,
}

pub async fn handle(ctx: &CliContext) -> Result<(), CliError> {
    let lock_path = if let Ok(data_dir) = std::env::var("AEROFS_DATA_DIR") {
        PathBuf::from(data_dir).join("aerofs.lock")
    } else {
        PathBuf::from("./aerofs.lock")
    };

    let status = DaemonLock::inspect_status(
        &lock_path,
        &ctx.config.server.host,
        ctx.config.server.port,
    );

    let output_data = StatusOutput {
        status: status.clone(),
        configured_host: ctx.config.server.host.clone(),
        configured_port: ctx.config.server.port,
        storage_root: ctx.config.filesystem.default_local_root.display().to_string(),
    };

    let human_status = || {
        println!("AeroFS Runtime Status:");
        println!("  • Configured Listen: {}:{}", ctx.config.server.host, ctx.config.server.port);
        println!("  • Storage Root: {}", ctx.config.filesystem.default_local_root.display());
        match &status {
            ProcessStatus::Running { pid, endpoint, lock_file } => {
                println!("  • State: RUNNING (PID: {})", pid);
                println!("  • Active Endpoint: http://{}", endpoint);
                println!("  • Lock File: {}", lock_file);
            }
            ProcessStatus::Stopped => {
                println!("  • State: STOPPED (No active daemon process)");
            }
            ProcessStatus::Stale { stale_pid, lock_file, message } => {
                println!("  • State: STALE LOCK (Previous PID: {})", stale_pid);
                println!("  • Lock File: {}", lock_file);
                println!("  • Warning: {}", message);
            }
            ProcessStatus::Unhealthy { pid, endpoint, reason } => {
                println!("  • State: UNHEALTHY (PID: {})", pid);
                println!("  • Endpoint: http://{}", endpoint);
                println!("  • Diagnostic: {}", reason);
            }
        }
    };

    match status {
        ProcessStatus::Running { .. } => {
            ctx.output.print_success("status", &output_data, human_status);
            Ok(())
        }
        _ => {
            let err = CliError::new(
                ExitCode::DaemonNotRunning,
                "DAEMON_NOT_RUNNING",
                "AeroFS daemon is not running",
            );
            ctx.output.print_failure("status", &output_data, &err, human_status);
            Err(err)
        }
    }
}
