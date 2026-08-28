pub mod args;
pub mod commands;
pub mod context;
pub mod daemon_lock;
pub mod error;
pub mod output;

pub use args::{Cli, Commands, ServeArgs};
pub use context::CliContext;
pub use daemon_lock::{DaemonLock, ProcessStatus};
pub use error::{CliError, ExitCode};
pub use output::{JsonEnvelope, OutputFormatter, OutputMode};

/// Main CLI execution entry point for non-daemon management commands
pub async fn run_cli(cli: Cli) -> Result<(), CliError> {
    let command_name = match &cli.command {
        Some(Commands::Status) => "status",
        Some(Commands::Version) => "version",
        Some(Commands::Doctor(_)) => "doctor",
        Some(Commands::Config(_)) => "config",
        Some(Commands::Db(_)) => "db",
        Some(Commands::Transfer(_)) => "transfer",
        Some(Commands::User(_)) => "user",
        Some(Commands::Connection(_)) => "connection",
        _ => "cli",
    };

    let ctx = match CliContext::from_cli(&cli) {
        Ok(c) => c,
        Err(e) => {
            let output = OutputFormatter::new(cli.json, cli.quiet, cli.verbose);
            output.print_error(command_name, &e);
            return Err(e);
        }
    };

    let result = match cli.command {
        None | Some(Commands::Serve(..)) => {
            // Serve daemon lifecycle is handled in main.rs
            Ok(())
        }
        Some(Commands::Status) => commands::status::handle(&ctx).await,
        Some(Commands::Version) => commands::version::handle(&ctx).await,
        Some(Commands::Doctor(args)) => commands::doctor::handle(args, &ctx).await,
        Some(Commands::Config(cmd)) => commands::config::handle(cmd, &ctx).await,
        Some(Commands::Db(cmd)) => commands::db::handle(cmd, &ctx).await,
        Some(Commands::Transfer(cmd)) => commands::transfer::handle(cmd, &ctx).await,
        Some(Commands::User(cmd)) => commands::user::handle(cmd, &ctx).await,
        Some(Commands::Connection(cmd)) => commands::connection::handle(cmd, &ctx).await,
    };

    if let Err(ref err) = result {
        ctx.output.print_error(command_name, err);
    }

    result
}
