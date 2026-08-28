use crate::cli::args::Cli;
use crate::cli::error::{CliError, ExitCode};
use crate::cli::output::OutputFormatter;
use crate::config::AppConfig;
use crate::db::{connect_db, DbPool};
use crate::state::AppState;
use std::path::PathBuf;

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
                format!("Failed to connect to database at {}: {}", self.config.database.url, e),
            )
        })
    }

    /// Construct full AppState with providers registry and transfer manager
    pub async fn state(&self) -> Result<AppState, CliError> {
        let pool = self.db().await?;
        let state = AppState::new_with_db(self.config.clone(), pool).await;
        Ok(state)
    }
}
