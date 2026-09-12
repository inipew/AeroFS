use crate::bootstrap::build_application;
use crate::cli::args::{Cli, ServeArgs};
use crate::cli::daemon_lock::DaemonLock;
use crate::config::AppConfig;
use crate::create_router;
use crate::db::init_db;
use crate::state::{RuntimeOwner, RuntimePhase, ShutdownReason};
use std::net::SocketAddr;
use tokio::net::TcpListener;

pub async fn run_server(cli: Cli, args: ServeArgs) -> anyhow::Result<()> {
    let lock_path = if let Ok(data_dir) = std::env::var("AEROFS_DATA_DIR") {
        std::path::PathBuf::from(data_dir).join("aerofs.lock")
    } else {
        std::path::PathBuf::from("./aerofs.lock")
    };
    let daemon_lock = DaemonLock::acquire(&lock_path)?;

    let mut config = AppConfig::load(cli.config.as_deref())?;
    if let Some(host) = args.host {
        config.server.host = host;
    }
    if let Some(port) = args.port {
        config.server.port = port;
    }

    let ip_addr: std::net::IpAddr = config.server.host.parse().unwrap_or_else(|_| {
        tracing::warn!(
            "Invalid host '{}', defaulting to 127.0.0.1",
            config.server.host
        );
        std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1))
    });
    let addr = SocketAddr::from((ip_addr, config.server.port));

    if !config.filesystem.default_local_root.exists() {
        std::fs::create_dir_all(&config.filesystem.default_local_root)?;
        tracing::info!(
            "Created storage directory: {:?}",
            config.filesystem.default_local_root
        );
    }
    if let Some(ref temp_dir) = config.filesystem.temp_dir {
        if !temp_dir.exists() {
            let _ = std::fs::create_dir_all(temp_dir);
        }
        std::env::set_var("TMPDIR", temp_dir);
    }

    let db = init_db(&config.database.url).await?;
    tracing::info!(
        "Database initialized successfully at {}",
        config.database.url
    );

    tracing::info!("runtime.phase=starting");
    let built = build_application(config, db).await;
    let state = built.state;
    let runtime = built.runtime;
    let app = create_router(state);

    runtime.set_phase(RuntimePhase::Binding);
    let listener = TcpListener::bind(addr).await?;
    runtime.set_phase(RuntimePhase::Running);

    if !cli.quiet {
        println!("🚀 AeroFS server listening on http://{}", addr);
    }
    tracing::info!("🚀 AeroFS server listening on http://{}", addr);

    let shutdown_token = runtime.shutdown_token.clone();
    let force_shutdown_token = runtime.force_shutdown_token.clone();
    let task_tracker = runtime.task_tracker.clone();

    let (drain_tx, drain_rx) = tokio::sync::oneshot::channel::<()>();
    let shutdown_start_time =
        std::sync::Arc::new(tokio::sync::Mutex::new(None::<std::time::Instant>));
    let shutdown_start_time_clone = std::sync::Arc::clone(&shutdown_start_time);
    let force_token_clone = force_shutdown_token.clone();

    let runtime_signal = runtime.clone();
    let shutdown_token_clone = shutdown_token.clone();
    tokio::spawn(async move {
        tokio::select! {
            _ = shutdown_signal(runtime_signal.clone()) => {}
            _ = shutdown_token_clone.cancelled() => {
                runtime_signal.request_shutdown(ShutdownReason::Internal);
            }
        }

        {
            let mut t = shutdown_start_time_clone.lock().await;
            *t = Some(std::time::Instant::now());
        }

        tokio::time::sleep(std::time::Duration::from_secs(15)).await;
        force_token_clone.cancel();
        let _ = drain_tx.send(());
    });

    let shutdown_fut = {
        let token = shutdown_token.clone();
        async move {
            token.cancelled().await;
        }
    };

    tokio::select! {
        res = axum::serve(listener, app).with_graceful_shutdown(shutdown_fut) => {
            if let Err(e) = res {
                tracing::error!("Server error: {}", e);
            }
        }
        _ = drain_rx => {
            tracing::warn!("runtime.phase=force_shutdown: Global shutdown deadline (15s) reached; forcing server stop");
        }
    }

    let elapsed = {
        let t = shutdown_start_time.lock().await;
        t.map(|start| start.elapsed()).unwrap_or_default()
    };
    let global_limit = std::time::Duration::from_secs(15);
    let shutdown_reason = runtime
        .shutdown_reason()
        .map(|r| r.as_str())
        .unwrap_or("internal");
    tracing::info!(
        "runtime.phase=draining: reason={} elapsed={:?}",
        shutdown_reason,
        elapsed
    );

    task_tracker.close();
    let remaining_time = global_limit.saturating_sub(elapsed);
    if !remaining_time.is_zero() {
        if (tokio::time::timeout(remaining_time, task_tracker.wait()).await).is_err() {
            tracing::warn!("Background task tracker drain timed out within remaining grace window");
        }
    } else {
        tracing::warn!(
            "Global shutdown deadline already exhausted; skipping extended background drain"
        );
    }

    runtime.set_phase(RuntimePhase::Stopped);
    tracing::info!("runtime.phase=stopped: released background workers, cleaning lock file...");
    daemon_lock.release();
    Ok(())
}

async fn shutdown_signal(runtime: RuntimeOwner) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
        ShutdownReason::CtrlC
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
        ShutdownReason::Sigterm
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<ShutdownReason>();

    let reason = tokio::select! {
        r = ctrl_c => {
            tracing::info!("Received Ctrl+C (SIGINT) shutdown signal. Starting graceful shutdown...");
            r
        },
        r = terminate => {
            tracing::info!("Received SIGTERM shutdown signal. Starting graceful shutdown...");
            r
        },
    };

    runtime.request_shutdown(reason);

    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            eprintln!("\n⚠️ Received second Ctrl+C. Forcing immediate exit.");
            std::process::exit(130);
        }
    });
}
