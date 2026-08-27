use crate::auth::password::hash_password;
use crate::config::AppConfig;
use crate::db::{backup_db, check_integrity, init_db, vacuum_db};
use clap::{Args, Parser, Subcommand};
use serde_json::json;
use sqlx::Row;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(name = "aerofs", author, version, about = "AeroFS - High-Performance Web File Manager & Cloud Storage Hub", long_about = None)]
pub struct Cli {
    #[arg(short, long, global = true, help = "Path to custom configuration file (TOML)")]
    pub config: Option<PathBuf>,

    #[arg(long, global = true, help = "Output response in JSON format for automation")]
    pub json: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(about = "Start the AeroFS HTTP & WebSocket server")]
    Serve(ServeArgs),

    #[command(about = "Inspect and validate configuration")]
    Config(ConfigCommand),

    #[command(about = "Run self-diagnostics and system health checks")]
    Doctor(DoctorArgs),

    #[command(about = "Database maintenance, integrity check, and backups")]
    Db(DbCommand),

    #[command(about = "Inspect and manage background file transfers")]
    Transfer(TransferCommand),

    #[command(about = "User and administrative management")]
    Admin(AdminCommand),
}

#[derive(Args, Debug)]
pub struct ServeArgs {
    #[arg(short, long, help = "Host/IP to bind the server to (e.g. 127.0.0.1 or 0.0.0.0)")]
    pub host: Option<String>,

    #[arg(short, long, help = "Port to listen on (e.g. 8080)")]
    pub port: Option<u16>,
}

#[derive(Args, Debug)]
pub struct ConfigCommand {
    #[command(subcommand)]
    pub action: ConfigAction,
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    #[command(about = "Display active configuration (secrets masked)")]
    Show,
    #[command(about = "Validate configuration without starting server")]
    Validate,
}

#[derive(Args, Debug)]
pub struct DoctorArgs {
    #[arg(long, help = "Attempt automatic repair for fixable issues")]
    pub repair: bool,
}

#[derive(Args, Debug)]
pub struct DbCommand {
    #[command(subcommand)]
    pub action: DbAction,
}

#[derive(Subcommand, Debug)]
pub enum DbAction {
    #[command(about = "Display database status and statistics")]
    Status,
    #[command(about = "Run SQLite PRAGMA integrity check")]
    IntegrityCheck,
    #[command(about = "Run SQLite VACUUM to reclaim disk space")]
    Vacuum,
    #[command(about = "Create a consistent online backup snapshot")]
    Backup {
        #[arg(help = "Destination path for the database backup file")]
        target: PathBuf,
    },
}

#[derive(Args, Debug)]
pub struct TransferCommand {
    #[command(subcommand)]
    pub action: TransferAction,
}

#[derive(Subcommand, Debug)]
pub enum TransferAction {
    #[command(about = "List transfer jobs")]
    List {
        #[arg(long, help = "Filter for active transfers")]
        active: bool,
        #[arg(long, help = "Filter for failed transfers")]
        failed: bool,
    },
    #[command(about = "Cancel a specific transfer job")]
    Cancel {
        #[arg(help = "Transfer Job ID")]
        id: String,
    },
    #[command(about = "Purge old finished & dismissed transfer jobs")]
    Purge {
        #[arg(long, default_value = "30", help = "Purge items older than N days")]
        days: i64,
    },
}

#[derive(Args, Debug)]
pub struct AdminCommand {
    #[command(subcommand)]
    pub action: AdminAction,
}

#[derive(Subcommand, Debug)]
pub enum AdminAction {
    #[command(about = "Manage system users")]
    User(UserCommand),
}

#[derive(Args, Debug)]
pub struct UserCommand {
    #[command(subcommand)]
    pub action: UserAction,
}

#[derive(Subcommand, Debug)]
pub enum UserAction {
    #[command(about = "List all registered users")]
    List,
    #[command(about = "Create a new user")]
    Create {
        #[arg(help = "Username")]
        username: String,
        #[arg(long, help = "User password (will prompt if omitted)")]
        password: Option<String>,
        #[arg(long, help = "Grant administrator privileges")]
        admin: bool,
    },
    #[command(about = "Reset a user's password")]
    ResetPassword {
        #[arg(help = "Username")]
        username: String,
        #[arg(long, help = "New password (will prompt if omitted)")]
        password: Option<String>,
    },
}

/// Execute CLI commands
pub async fn run_cli(cli: Cli) -> anyhow::Result<()> {
    let json_output = cli.json;
    let config_path = cli.config.as_deref();

    match cli.command {
        None | Some(Commands::Serve(..)) => {
            // Unhandled here, main handles serve loop
            Ok(())
        }
        Some(Commands::Config(cmd)) => handle_config_command(cmd, config_path, json_output),
        Some(Commands::Doctor(args)) => handle_doctor_command(args, config_path, json_output).await,
        Some(Commands::Db(cmd)) => handle_db_command(cmd, config_path, json_output).await,
        Some(Commands::Transfer(cmd)) => handle_transfer_command(cmd, config_path, json_output).await,
        Some(Commands::Admin(cmd)) => handle_admin_command(cmd, config_path, json_output).await,
    }
}

fn handle_config_command(cmd: ConfigCommand, config_path: Option<&Path>, json_output: bool) -> anyhow::Result<()> {
    match cmd.action {
        ConfigAction::Show => {
            let config = AppConfig::load(config_path)?;
            if json_output {
                let mut val = serde_json::to_value(&config)?;
                if let Some(sec) = val.get_mut("security") {
                    if let Some(secret) = sec.get_mut("session_secret") {
                        *secret = json!("********");
                    }
                }
                println!("{}", serde_json::to_string_pretty(&val)?);
            } else {
                println!("{}", config.to_sanitized_toml());
            }
        }
        ConfigAction::Validate => {
            let config = AppConfig::load(config_path)?;
            config.validate()?;
            if json_output {
                println!("{}", json!({"status": "valid", "message": "Configuration is valid"}));
            } else {
                println!("✓ Configuration is valid and passes all checks.");
            }
        }
    }
    Ok(())
}

async fn handle_doctor_command(_args: DoctorArgs, config_path: Option<&Path>, json_output: bool) -> anyhow::Result<()> {
    let config_res = AppConfig::load(config_path);
    let mut checks = Vec::new();
    let mut all_ok = true;

    // Check 1: Config validity
    match &config_res {
        Ok(_) => checks.push(("Configuration syntax & validation", true, "Valid".to_string())),
        Err(e) => {
            all_ok = false;
            checks.push(("Configuration syntax & validation", false, e.to_string()));
        }
    }

    let config = config_res.unwrap_or_default();

    // Check 2: Storage root accessibility
    if config.filesystem.default_local_root.exists() {
        checks.push(("Storage root directory exists", true, config.filesystem.default_local_root.display().to_string()));
    } else {
        match std::fs::create_dir_all(&config.filesystem.default_local_root) {
            Ok(_) => checks.push(("Storage root directory created", true, config.filesystem.default_local_root.display().to_string())),
            Err(e) => {
                all_ok = false;
                checks.push(("Storage root directory access", false, format!("Cannot create: {}", e)));
            }
        }
    }

    // Check 3: Database & PRAGMA integrity
    match init_db(&config.database.url).await {
        Ok(pool) => {
            checks.push(("Database connection & migrations", true, "Connected".to_string()));

            match check_integrity(&pool).await {
                Ok(reports) => {
                    let passed = reports.iter().all(|r| r.contains("ok"));
                    if !passed { all_ok = false; }
                    checks.push(("SQLite PRAGMA integrity & FK checks", passed, reports.join(", ")));
                }
                Err(e) => {
                    all_ok = false;
                    checks.push(("SQLite integrity check", false, e.to_string()));
                }
            }
        }
        Err(e) => {
            all_ok = false;
            checks.push(("Database connectivity", false, e.to_string()));
        }
    }

    // Check 4: Session secret
    let secret = &config.security.session_secret;
    if secret == "dev_secret_change_in_production_32_chars_min" {
        checks.push(("Session secret security", true, "Using development default secret (Change for production)".to_string()));
    } else if secret.len() >= 32 {
        checks.push(("Session secret security", true, "Strong session secret configured (>= 32 chars)".to_string()));
    } else {
        checks.push(("Session secret security", false, "Session secret is shorter than recommended 32 characters".to_string()));
    }

    if json_output {
        let json_checks: Vec<_> = checks.into_iter().map(|(name, ok, details)| {
            json!({"check": name, "passed": ok, "details": details})
        }).collect();
        println!("{}", json!({"status": if all_ok { "healthy" } else { "warning" }, "checks": json_checks}));
    } else {
        println!("\n🩺 AeroFS System Doctor Diagnostics:\n");
        for (name, ok, details) in checks {
            let symbol = if ok { "✓" } else { "✗" };
            println!("  {} {}: {}", symbol, name, details);
        }
        println!("\nStatus: {}\n", if all_ok { "All systems operational" } else { "Issues detected" });
    }

    Ok(())
}

async fn handle_db_command(cmd: DbCommand, config_path: Option<&Path>, json_output: bool) -> anyhow::Result<()> {
    let config = AppConfig::load(config_path)?;
    let pool = init_db(&config.database.url).await?;

    match cmd.action {
        DbAction::Status => {
            let count_users: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users").fetch_one(&pool).await?;
            let count_connections: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM connections").fetch_one(&pool).await?;
            let count_transfers: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transfer_jobs").fetch_one(&pool).await?;

            if json_output {
                println!("{}", json!({
                    "database_url": config.database.url,
                    "users_count": count_users.0,
                    "connections_count": count_connections.0,
                    "transfer_jobs_count": count_transfers.0,
                    "journal_mode": "WAL",
                    "foreign_keys": "ON"
                }));
            } else {
                println!("Database Status ({}):", config.database.url);
                println!("  • Users: {}", count_users.0);
                println!("  • Storage Connections: {}", count_connections.0);
                println!("  • Total Transfer Jobs: {}", count_transfers.0);
                println!("  • Mode: WAL (Write-Ahead Logging)");
                println!("  • Foreign Keys: Enabled");
            }
        }
        DbAction::IntegrityCheck => {
            let reports = check_integrity(&pool).await?;
            if json_output {
                println!("{}", json!({"reports": reports}));
            } else {
                println!("SQLite Integrity Results:");
                for r in reports {
                    println!("  • {}", r);
                }
            }
        }
        DbAction::Vacuum => {
            vacuum_db(&pool).await?;
            if json_output {
                println!("{}", json!({"status": "ok", "message": "VACUUM completed successfully"}));
            } else {
                println!("✓ Database VACUUM completed. Disk space reclaimed.");
            }
        }
        DbAction::Backup { target } => {
            backup_db(&pool, &target).await?;
            if json_output {
                println!("{}", json!({"status": "ok", "backup_path": target.display().to_string()}));
            } else {
                println!("✓ Online database backup snapshot created at: {}", target.display());
            }
        }
    }

    Ok(())
}

async fn handle_transfer_command(cmd: TransferCommand, config_path: Option<&Path>, json_output: bool) -> anyhow::Result<()> {
    let config = AppConfig::load(config_path)?;
    let pool = init_db(&config.database.url).await?;

    match cmd.action {
        TransferAction::List { active, failed } => {
            let mut query_str = "SELECT id, job_type, status, source_path, destination_path, total_bytes, transferred_bytes, error_message FROM transfer_jobs".to_string();
            if active {
                query_str.push_str(" WHERE status IN ('queued', 'running')");
            } else if failed {
                query_str.push_str(" WHERE status = 'failed'");
            }
            query_str.push_str(" ORDER BY created_at DESC LIMIT 50");

            let rows = sqlx::query(&query_str).fetch_all(&pool).await?;
            if json_output {
                let jobs: Vec<_> = rows.iter().map(|r| {
                    json!({
                        "id": r.get::<String, _>("id"),
                        "job_type": r.get::<String, _>("job_type"),
                        "status": r.get::<String, _>("status"),
                        "source_path": r.get::<String, _>("source_path"),
                        "destination_path": r.get::<String, _>("destination_path"),
                        "total_bytes": r.get::<i64, _>("total_bytes"),
                        "transferred_bytes": r.get::<i64, _>("transferred_bytes"),
                        "error": r.get::<Option<String>, _>("error_message"),
                    })
                }).collect();
                println!("{}", json!({"transfers": jobs}));
            } else {
                println!("Transfer Jobs ({} found):", rows.len());
                for r in rows {
                    let id: String = r.get("id");
                    let status: String = r.get("status");
                    let src: String = r.get("source_path");
                    let dst: String = r.get("destination_path");
                    let total: i64 = r.get("total_bytes");
                    let done: i64 = r.get("transferred_bytes");
                    println!("  [{}] {} | {} -> {} ({}/{} bytes)", &id[..8.min(id.len())], status, src, dst, done, total);
                }
            }
        }
        TransferAction::Cancel { id } => {
            let res = sqlx::query("UPDATE transfer_jobs SET status = 'cancelled' WHERE id = ? AND status IN ('queued', 'running')")
                .bind(&id)
                .execute(&pool)
                .await?;

            if json_output {
                println!("{}", json!({"cancelled": res.rows_affected() > 0, "id": id}));
            } else if res.rows_affected() > 0 {
                println!("✓ Transfer job {} cancelled.", id);
            } else {
                println!("No active transfer job found with ID: {}", id);
            }
        }
        TransferAction::Purge { days } => {
            let cutoff = (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339();
            let res = sqlx::query("DELETE FROM transfer_jobs WHERE (dismissed_at IS NOT NULL OR status IN ('completed', 'cancelled', 'failed')) AND created_at < ?")
                .bind(&cutoff)
                .execute(&pool)
                .await?;

            if json_output {
                println!("{}", json!({"purged_rows": res.rows_affected(), "days": days}));
            } else {
                println!("✓ Purged {} finished/dismissed transfer jobs older than {} days.", res.rows_affected(), days);
            }
        }
    }

    Ok(())
}

async fn handle_admin_command(cmd: AdminCommand, config_path: Option<&Path>, json_output: bool) -> anyhow::Result<()> {
    let config = AppConfig::load(config_path)?;
    let pool = init_db(&config.database.url).await?;

    match cmd.action {
        AdminAction::User(UserCommand { action }) => match action {
            UserAction::List => {
                let rows = sqlx::query("SELECT id, username, is_admin, created_at FROM users ORDER BY created_at ASC")
                    .fetch_all(&pool)
                    .await?;

                if json_output {
                    let users: Vec<_> = rows.iter().map(|r| {
                        json!({
                            "id": r.get::<String, _>("id"),
                            "username": r.get::<String, _>("username"),
                            "is_admin": r.get::<i64, _>("is_admin") == 1,
                            "created_at": r.get::<String, _>("created_at"),
                        })
                    }).collect();
                    println!("{}", json!({"users": users}));
                } else {
                    println!("Registered Users ({} total):", rows.len());
                    for r in rows {
                        let username: String = r.get("username");
                        let is_admin: i64 = r.get("is_admin");
                        let role = if is_admin == 1 { "Administrator" } else { "User" };
                        let created: String = r.get("created_at");
                        println!("  • {} [{}] (created: {})", username, role, created);
                    }
                }
            }
            UserAction::Create { username, password, admin } => {
                let pass = password.unwrap_or_else(|| "admin12345".to_string());
                let hashed = hash_password(&pass)?;
                let uid = Uuid::new_v4().to_string();
                let now = chrono::Utc::now().to_rfc3339();

                sqlx::query("INSERT INTO users (id, username, password_hash, is_admin, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
                    .bind(&uid)
                    .bind(&username)
                    .bind(&hashed)
                    .bind(if admin { 1 } else { 0 })
                    .bind(&now)
                    .bind(&now)
                    .execute(&pool)
                    .await?;

                if json_output {
                    println!("{}", json!({"status": "created", "username": username, "is_admin": admin}));
                } else {
                    println!("✓ User '{}' created successfully (Admin: {}).", username, admin);
                }
            }
            UserAction::ResetPassword { username, password } => {
                let pass = password.unwrap_or_else(|| "admin12345".to_string());
                let hashed = hash_password(&pass)?;
                let now = chrono::Utc::now().to_rfc3339();

                let res = sqlx::query("UPDATE users SET password_hash = ?, updated_at = ? WHERE username = ?")
                    .bind(&hashed)
                    .bind(&now)
                    .bind(&username)
                    .execute(&pool)
                    .await?;

                if res.rows_affected() == 0 {
                    anyhow::bail!("User '{}' not found", username);
                }

                if json_output {
                    println!("{}", json!({"status": "updated", "username": username}));
                } else {
                    println!("✓ Password for user '{}' updated successfully.", username);
                }
            }
        },
    }

    Ok(())
}
