use crate::bootstrap::build_application;
use crate::cli::args::Cli;
use crate::cli::error::{CliError, ExitCode};
use crate::cli::output::OutputFormatter;
use crate::config::AppConfig;
use crate::db::{connect_db, DbPool};
use crate::state::{AppState, RuntimeOwner, ShutdownReason};
use std::ops::Deref;
use std::path::PathBuf;

/// Scoped full-application handle for one-shot CLI commands.
///
/// CLI commands need request-facing `AppState`, but bootstrapping that state also
/// starts supervised background workers. Keep the process-owned `RuntimeOwner`
/// alive for exactly as long as the CLI command uses the state and signal those
/// workers to stop when the guard leaves scope.
pub struct CliState {
    state: AppState,
    runtime: RuntimeOwner,
}

impl Deref for CliState {
    type Target = AppState;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl Drop for CliState {
    fn drop(&mut self) {
        self.runtime.request_shutdown(ShutdownReason::Manual);
    }
}

pub struct CliContext {
    pub config: AppConfig,
    pub config_path: Option<PathBuf>,
    pub output: OutputFormatter,
}

impl CliContext {
    pub fn from_cli(cli: &Cli) -> Result<Self, CliError> {
        let config = AppConfig::load(cli.config.as_deref()).map_err(|e| {
            CliError::new(
                ExitCode::ConfigError,
                "CONFIG_ERROR",
                format!("Failed to load configuration: {}", e),
            )
        })?;

        let output = OutputFormatter::new(cli.json, cli.quiet, cli.verbose);

        Ok(Self {
            config,
            config_path: cli.config.clone(),
            output,
        })
    }

    /// Connect to SQLite without performing auto-migrations or seeds (safe for diagnostics)
    pub async fn db(&self) -> Result<DbPool, CliError> {
        connect_db(&self.config.database.url).await.map_err(|e| {
            CliError::new(
                ExitCode::DatabaseError,
                "DATABASE_CONNECT_ERROR",
                format!(
                    "Failed to connect to database at {}: {}",
                    self.config.database.url, e
                ),
            )
        })
    }

    /// Construct the full application for a one-shot CLI command while retaining
    /// ownership of all runtime tasks and cancellation handles for the guard lifetime.
    pub async fn state(&self) -> Result<CliState, CliError> {
        let pool = self.db().await?;
        let built = build_application(self.config.clone(), pool).await;
        Ok(CliState {
            state: built.state,
            runtime: built.runtime,
        })
    }
}
