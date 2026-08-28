use backend::cli::commands::serve::run_server;
use backend::cli::{run_cli, Cli, Commands, ServeArgs};
use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // 1. Determine logging filter based on global CLI flags
    let is_serve = matches!(cli.command, None | Some(Commands::Serve(..)));
    let default_filter = if let Some(ref lvl) = cli.log_level {
        format!("backend={},tower_http={}", lvl, lvl)
    } else if cli.verbose {
        "backend=debug,tower_http=debug".to_string()
    } else if cli.quiet || !is_serve {
        "backend=warn,tower_http=warn".to_string()
    } else {
        std::env::var("RUST_LOG").unwrap_or_else(|_| "backend=info,tower_http=info".into())
    };

    // Initialize structured logging / tracing subscriber
    let env_filter = tracing_subscriber::EnvFilter::try_new(&default_filter)
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 2. Dispatch command to daemon lifecycle or one-shot management CLI
    match cli.command.clone() {
        Some(Commands::Serve(args)) => {
            if let Err(e) = run_server(cli, args).await {
                eprintln!("❌ Error: {}", e);
                std::process::exit(if e.to_string().contains("already running") {
                    backend::cli::ExitCode::DaemonAlreadyRunning.as_i32()
                } else {
                    backend::cli::ExitCode::GeneralError.as_i32()
                });
            }
        }
        None => {
            if let Err(e) = run_server(cli, ServeArgs::default()).await {
                eprintln!("❌ Error: {}", e);
                std::process::exit(if e.to_string().contains("already running") {
                    backend::cli::ExitCode::DaemonAlreadyRunning.as_i32()
                } else {
                    backend::cli::ExitCode::GeneralError.as_i32()
                });
            }
        }
        Some(_) => {
            if let Err(e) = run_cli(cli).await {
                std::process::exit(e.code.as_i32());
            }
        }
    }

    Ok(())
}
