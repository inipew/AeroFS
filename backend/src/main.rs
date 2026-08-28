use backend::cli::{Cli, Commands};
use backend::{config::AppConfig, create_router, db::init_db, AppState};
use clap::Parser;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // If a management subcommand is invoked, execute and exit
    if let Some(ref cmd) = cli.command {
        match cmd {
            Commands::Config(..)
            | Commands::Status
            | Commands::Doctor(..)
            | Commands::Db(..)
            | Commands::Transfer(..)
            | Commands::Admin(..) => {
                return backend::cli::run_cli(cli).await;
            }
            Commands::Serve(..) => {}
        }
    }

    // Initialize tracing logger for server mode
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "backend=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 0. Acquire singleton daemon lock file
    let lock_path = if let Ok(data_dir) = std::env::var("AEROFS_DATA_DIR") {
        std::path::PathBuf::from(data_dir).join("aerofs.lock")
    } else {
        std::path::PathBuf::from("./aerofs.lock")
    };

    let pid = std::process::id();
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&lock_path)
    {
        use std::io::Write;
        let _ = write!(f, "{}", pid);
    }

    // 1. Hierarchically load configuration (CLI > Env > TOML > Defaults)
    let mut config = AppConfig::load(cli.config.as_deref())?;

    // 2. Apply serve CLI argument overrides if provided
    if let Some(Commands::Serve(args)) = cli.command {
        if let Some(host) = args.host {
            config.server.host = host;
        }
        if let Some(port) = args.port {
            config.server.port = port;
        }
    }

    let ip_addr: std::net::IpAddr = config.server.host.parse().unwrap_or_else(|_| {
        tracing::warn!("Invalid host '{}', defaulting to 127.0.0.1", config.server.host);
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

    // 4. Initialize SQLite pool with WAL mode, foreign keys, and run migrations
    let db = init_db(&config.database.url).await?;
    tracing::info!("Database initialized successfully at {}", config.database.url);

    let state = AppState::new_with_db(config, db).await;

    // 5. Background housekeeping worker: cleans up expired sessions & old dismissed jobs every hour
    let housekeeping_db = state.db.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
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
    });

    let app = create_router(state);

    tracing::info!("🚀 AeroFS server listening on http://{}", addr);
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    if lock_path.exists() {
        let _ = std::fs::remove_file(&lock_path);
    }

    Ok(())
}

async fn shutdown_signal() {
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
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutdown signal received, starting graceful shutdown");
}
