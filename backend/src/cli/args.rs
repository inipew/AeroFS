use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "aerofs",
    author,
    version,
    about = "AeroFS - High-Performance Web File Manager & Cloud Storage Hub",
    long_about = None
)]
pub struct Cli {
    #[arg(
        short,
        long,
        global = true,
        help = "Path to custom configuration file (TOML)"
    )]
    pub config: Option<PathBuf>,

    #[arg(
        long,
        global = true,
        help = "Output response in standardized JSON envelope format"
    )]
    pub json: bool,

    #[arg(
        short,
        long,
        global = true,
        help = "Suppress decorative headers and non-essential output"
    )]
    pub quiet: bool,

    #[arg(
        short,
        long,
        global = true,
        help = "Enable verbose debug logs and detailed context"
    )]
    pub verbose: bool,

    #[arg(
        long,
        global = true,
        help = "Set log tracing level (trace, debug, info, warn, error)"
    )]
    pub log_level: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    #[command(about = "Start the AeroFS HTTP & WebSocket server daemon")]
    Serve(ServeArgs),

    #[command(about = "Display daemon process and runtime status")]
    Status,

    #[command(about = "Display detailed build, version, and target information")]
    Version,

    #[command(about = "Run self-diagnostics and system health checks")]
    Doctor(DoctorArgs),

    #[command(about = "Inspect and validate configuration")]
    Config(ConfigCommand),

    #[command(about = "Database maintenance, migration, integrity, and backups")]
    Db(DbCommand),

    #[command(about = "Inspect and manage background file transfers")]
    Transfer(TransferCommand),

    #[command(about = "User and account administrative management")]
    User(UserCommand),

    #[command(about = "Inspect and test storage provider connections")]
    Connection(ConnectionCommand),
}

#[derive(Args, Debug, Default, Clone)]
pub struct ServeArgs {
    #[arg(
        short,
        long,
        help = "Host/IP to bind the server to (e.g. 127.0.0.1 or 0.0.0.0)"
    )]
    pub host: Option<String>,

    #[arg(short, long, help = "TCP port to listen on (e.g. 8080)")]
    pub port: Option<u16>,
}

#[derive(Args, Debug, Clone)]
pub struct DoctorArgs {
    #[arg(long, help = "Attempt automatic repair for fixable issues")]
    pub repair: bool,

    #[arg(long, help = "Simulate repair operations without making changes")]
    pub dry_run: bool,

    #[arg(short, long, help = "Automatically confirm repair actions without prompt")]
    pub yes: bool,
}

#[derive(Args, Debug, Clone)]
pub struct ConfigCommand {
    #[command(subcommand)]
    pub action: ConfigAction,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ConfigAction {
    #[command(about = "Display active configuration (secrets masked)")]
    Show,

    #[command(about = "Display effective layered configuration with provenance sources")]
    Effective,

    #[command(about = "Get effective value of a specific configuration key")]
    Get {
        #[arg(help = "Configuration key path (e.g. server.port)")]
        key: String,
    },

    #[command(about = "Explain schema, default, and subsystem usage for a config key")]
    Explain {
        #[arg(help = "Configuration key path (e.g. limits.max_concurrent_transfers)")]
        key: String,
    },

    #[command(about = "Validate configuration file without starting server")]
    Validate,
}

#[derive(Args, Debug, Clone)]
pub struct DbCommand {
    #[command(subcommand)]
    pub action: DbAction,
}

#[derive(Subcommand, Debug, Clone)]
pub enum DbAction {
    #[command(about = "Display database status, statistics, and PRAGMA settings")]
    Status,

    #[command(about = "Run pending database migrations")]
    Migrate,

    #[command(about = "Run SQLite PRAGMA integrity_check and foreign_key_check", alias = "integrity-check")]
    Integrity,

    #[command(about = "Run SQLite VACUUM to reclaim disk space")]
    Vacuum,

    #[command(about = "Run SQLite WAL checkpoint to truncate journal file")]
    Checkpoint,

    #[command(about = "Create a consistent online backup snapshot")]
    Backup {
        #[arg(help = "Destination path for the database backup file")]
        target: PathBuf,

        #[arg(short, long, help = "Force overwrite destination if file already exists")]
        force: bool,
    },
}

#[derive(Args, Debug, Clone)]
pub struct TransferCommand {
    #[command(subcommand)]
    pub action: TransferAction,
}

#[derive(Subcommand, Debug, Clone)]
pub enum TransferAction {
    #[command(about = "List transfer jobs with filtering")]
    List {
        #[arg(long, help = "Filter by job status (queued, running, completed, failed, cancelled)")]
        status: Option<String>,

        #[arg(long, default_value = "50", help = "Maximum number of records to return")]
        limit: usize,

        #[arg(long, help = "Filter by username or user ID")]
        user: Option<String>,

        #[arg(long, help = "Filter by storage connection ID")]
        connection: Option<String>,
    },

    #[command(about = "Show detailed status and metrics for a transfer job")]
    Show {
        #[arg(help = "Transfer Job ID")]
        id: String,
    },

    #[command(about = "Cancel an active transfer job via TransferManager")]
    Cancel {
        #[arg(help = "Transfer Job ID")]
        id: String,
    },

    #[command(about = "Dismiss a finished transfer job from history")]
    Dismiss {
        #[arg(help = "Transfer Job ID")]
        id: String,
    },

    #[command(about = "Dismiss all completed/cancelled transfer jobs")]
    Clear,

    #[command(about = "Purge old finished & dismissed transfer records")]
    Purge {
        #[arg(long, default_value = "30", help = "Purge items older than N days (>= 1)")]
        days: u32,

        #[arg(long, help = "Simulate purge operation without deleting records")]
        dry_run: bool,

        #[arg(short, long, help = "Confirm purge without interactive prompt")]
        yes: bool,
    },

    #[command(about = "Repair stuck or interrupted transfer jobs after server crash")]
    Repair {
        #[arg(long, help = "Simulate repair without modifying database")]
        dry_run: bool,

        #[arg(short, long, help = "Confirm repair without interactive prompt")]
        yes: bool,
    },
}

#[derive(Args, Debug, Clone)]
pub struct UserCommand {
    #[command(subcommand)]
    pub action: UserAction,
}

#[derive(Subcommand, Debug, Clone)]
pub enum UserAction {
    #[command(about = "List all registered users")]
    List,

    #[command(about = "Display details for a specific user")]
    Show {
        #[arg(help = "Username")]
        username: String,
    },

    #[command(about = "Create a new user account")]
    Create {
        #[arg(help = "Username")]
        username: String,

        #[arg(long, help = "Grant administrator privileges")]
        admin: bool,

        #[arg(long, help = "Read password from stdin pipe instead of interactive prompt")]
        password_stdin: bool,
    },

    #[command(about = "Update user password", alias = "reset-password")]
    Passwd {
        #[arg(help = "Username")]
        username: String,

        #[arg(long, help = "Read new password from stdin pipe instead of interactive prompt")]
        password_stdin: bool,
    },

    #[command(about = "Delete a user account (with last administrator safeguard)")]
    Delete {
        #[arg(help = "Username")]
        username: String,

        #[arg(short, long, help = "Confirm deletion without interactive prompt")]
        yes: bool,
    },

    #[command(about = "Promote user to administrator")]
    Promote {
        #[arg(help = "Username")]
        username: String,
    },

    #[command(about = "Demote administrator to standard user")]
    Demote {
        #[arg(help = "Username")]
        username: String,
    },
}

#[derive(Args, Debug, Clone)]
pub struct ConnectionCommand {
    #[command(subcommand)]
    pub action: ConnectionAction,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ConnectionAction {
    #[command(about = "List all configured storage connections")]
    List,

    #[command(about = "Display details and capabilities for a connection")]
    Show {
        #[arg(help = "Connection ID")]
        id: String,
    },

    #[command(about = "Run multi-stage diagnostic connectivity test on a connection")]
    Test {
        #[arg(help = "Connection ID")]
        id: String,
    },

    #[command(about = "Enable a storage connection")]
    Enable {
        #[arg(help = "Connection ID")]
        id: String,
    },

    #[command(about = "Disable a storage connection")]
    Disable {
        #[arg(help = "Connection ID")]
        id: String,
    },
}
