use crate::cli::args::{Cli, ServeArgs};
use crate::cli::daemon_lock::DaemonLock;
use crate::config::AppConfig;
use crate::create_router;
use crate::db::init_db;
use crate::state::AppState;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

pub async fn run_server(cli: Cli, args: ServeArgs) -> anyhow::Result<()> {
    // 0. Resolve lock file path
    let lock_path = if let Ok(data_dir) = std::env::var("AEROFS_DATA_DIR") {
        std::path::PathBuf::from(data_dir).join("aerofs.lock")
    } else {
        std::path::PathBuf::from("./aerofs.lock")
    };

    // Acquire exclusive singleton daemon lock
    let daemon_lock = DaemonLock::acquire(&lock_path)?;

    // 1. Hierarchically load configuration
    let mut config = AppConfig::load(cli.config.as_deref())?;

    // 2. Apply serve CLI overrides if provided
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

    // 3. Ensure default local storage and temp directories exist
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

    // 4. Initialize SQLite pool with WAL mode, foreign keys, migrations, and seeds
    let db = init_db(&config.database.url).await?;
    tracing::info!(
        "Database initialized successfully at {}",
        config.database.url
    );

    let state = AppState::new_with_db(config, db).await;

    // 5. Background task cancellation & lifecycle coordination
    let cancel_token = CancellationToken::new();

    // Background housekeeping worker: cleans up expired sessions & old dismissed jobs every hour
    let housekeeping_db = state.db.clone();
    let housekeeping_token = cancel_token.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            tokio::select! {
                _ = housekeeping_token.cancelled() => {
                    tracing::info!("Housekeeping worker received shutdown cancellation");
                    break;
                }
                _ = interval.tick() => {
                    let now = chrono::Utc::now().to_rfc3339();
                    // Delete expired sessions
                    let del_sessions = sqlx::query("DELETE FROM sessions WHERE expires_at < ?")
                        .bind(&now)
                        .execute(&housekeeping_db)
                        .await;
                    if let Ok(res) = del_sessions {
                        if res.rows_affected() > 0 {
                            tracing::info!("Housekeeping: purged {} expired sessions", res.rows_affected());
                        }
                    }

                    // Delete dismissed transfer jobs older than 30 days
                    let del_transfers = sqlx::query(
                        "DELETE FROM transfer_jobs WHERE dismissed_at IS NOT NULL AND dismissed_at < datetime('now', '-30 days')",
                    )
                    .execute(&housekeeping_db)
                    .await;
                    if let Ok(res) = del_transfers {
                        if res.rows_affected() > 0 {
                            tracing::info!("Housekeeping: purged {} old dismissed transfer jobs", res.rows_affected());
                        }
                    }
                }
            }
        }
    });

    let app = create_router(state);

    if !cli.quiet {
        println!("🚀 AeroFS server listening on http://{}", addr);
    }
    tracing::info!("🚀 AeroFS server listening on http://{}", addr);

    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(cancel_token.clone()))
        .await?;

    // Signal all workers to stop and wait
    cancel_token.cancel();
    tracing::info!("Shutdown coordinator: released background workers, cleaning lock file...");

    daemon_lock.release();

    Ok(())
}

async fn shutdown_signal(cancel_token: CancellationToken) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("Received Ctrl+C (SIGINT) shutdown signal");
        },
        _ = terminate => {
            tracing::info!("Received SIGTERM shutdown signal");
        },
    }

    cancel_token.cancel();
}
