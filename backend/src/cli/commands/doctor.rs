use crate::cli::args::DoctorArgs;
use crate::cli::context::CliContext;
use crate::cli::daemon_lock::{DaemonLock, ProcessStatus};
use crate::cli::error::CliError;
use crate::cli::output::prompt_confirm;
use crate::db::{check_integrity, connect_db};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Ok,
    Info,
    Warning,
    Critical,
}

impl Severity {
    pub fn symbol(&self) -> &'static str {
        match self {
            Severity::Ok => "✓",
            Severity::Info => "ℹ",
            Severity::Warning => "⚠",
            Severity::Critical => "✗",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorCheck {
    pub category: &'static str,
    pub name: &'static str,
    pub severity: Severity,
    pub details: String,
    pub fixable: bool,
}

#[derive(Debug, Serialize)]
pub struct DoctorReport {
    pub overall_status: &'static str,
    pub ok_count: usize,
    pub warning_count: usize,
    pub critical_count: usize,
    pub checks: Vec<DoctorCheck>,
    pub repairs_applied: Vec<String>,
}

pub async fn handle(args: DoctorArgs, ctx: &CliContext) -> Result<(), CliError> {
    let mut checks = Vec::new();
    let mut repairs_applied = Vec::new();

    // ==========================================
    // Category 1: Runtime Liveness
    // ==========================================
    let lock_path = if let Ok(data_dir) = std::env::var("AEROFS_DATA_DIR") {
        PathBuf::from(data_dir).join("aerofs.lock")
    } else {
        PathBuf::from("./aerofs.lock")
    };

    let p_status = DaemonLock::inspect_status(&lock_path, &ctx.config.server.host, ctx.config.server.port);
    match &p_status {
        ProcessStatus::Running { pid, endpoint, .. } => {
            checks.push(DoctorCheck {
                category: "Runtime",
                name: "Daemon Process Liveness",
                severity: Severity::Ok,
                details: format!("Running with PID {} listening on http://{}", pid, endpoint),
                fixable: false,
            });
        }
        ProcessStatus::Stopped => {
            checks.push(DoctorCheck {
                category: "Runtime",
                name: "Daemon Process Liveness",
                severity: Severity::Info,
                details: "Daemon is currently stopped (offline management mode)".to_string(),
                fixable: false,
            });
        }
        ProcessStatus::Stale { stale_pid, message, .. } => {
            checks.push(DoctorCheck {
                category: "Runtime",
                name: "Daemon Process Liveness",
                severity: Severity::Warning,
                details: format!("Stale lock file detected (PID: {}): {}", stale_pid, message),
                fixable: true,
            });
            if args.repair {
                let should_fix = args.yes || prompt_confirm("Remove stale daemon lock file?", false).unwrap_or(false);
                if should_fix && !args.dry_run {
                    let _ = std::fs::remove_file(&lock_path);
                    repairs_applied.push("Removed stale daemon lock file".to_string());
                }
            }
        }
        ProcessStatus::Unhealthy { pid, endpoint, reason } => {
            checks.push(DoctorCheck {
                category: "Runtime",
                name: "Daemon Process Liveness",
                severity: Severity::Critical,
                details: format!("Process {} at {} is unhealthy: {}", pid, endpoint, reason),
                fixable: false,
            });
        }
    }

    // ==========================================
    // Category 2: Database Integrity & Tables
    // ==========================================
    match connect_db(&ctx.config.database.url).await {
        Ok(pool) => {
            checks.push(DoctorCheck {
                category: "Database",
                name: "SQLite Connection",
                severity: Severity::Ok,
                details: "Successfully connected to SQLite pool".to_string(),
                fixable: false,
            });

            // Integrity checks
            match check_integrity(&pool).await {
                Ok(reports) => {
                    let pass = reports.iter().all(|r| r.contains("ok"));
                    checks.push(DoctorCheck {
                        category: "Database",
                        name: "PRAGMA Integrity & Foreign Keys",
                        severity: if pass { Severity::Ok } else { Severity::Critical },
                        details: reports.join("; "),
                        fixable: false,
                    });
                }
                Err(e) => {
                    checks.push(DoctorCheck {
                        category: "Database",
                        name: "PRAGMA Integrity & Foreign Keys",
                        severity: Severity::Critical,
                        details: format!("Integrity check error: {}", e),
                        fixable: false,
                    });
                }
            }

            // Admin user check
            let admin_count: Result<(i64,), _> =
                sqlx::query_as("SELECT COUNT(*) FROM users WHERE is_admin = 1")
                    .fetch_one(&pool)
                    .await;

            match admin_count {
                Ok((c,)) if c > 0 => {
                    checks.push(DoctorCheck {
                        category: "Database",
                        name: "Administrator Accounts",
                        severity: Severity::Ok,
                        details: format!("{} administrator account(s) registered", c),
                        fixable: false,
                    });
                }
                Ok(_) => {
                    checks.push(DoctorCheck {
                        category: "Database",
                        name: "Administrator Accounts",
                        severity: Severity::Critical,
                        details: "No administrator accounts found. System has no administrator!".to_string(),
                        fixable: false,
                    });
                }
                Err(e) => {
                    checks.push(DoctorCheck {
                        category: "Database",
                        name: "Administrator Accounts",
                        severity: Severity::Warning,
                        details: format!("Users table not yet initialized: {}", e),
                        fixable: false,
                    });
                }
            }

            // ==========================================
            // Category 4: Transfer Subsystem Checks
            // ==========================================
            let stuck_jobs: Result<(i64,), _> =
                sqlx::query_as("SELECT COUNT(*) FROM transfer_jobs WHERE status IN ('running', 'cancellation_requested')")
                    .fetch_one(&pool)
                    .await;

            if let Ok((stuck,)) = stuck_jobs {
                if stuck > 0 && matches!(p_status, ProcessStatus::Stopped | ProcessStatus::Stale { .. }) {
                    checks.push(DoctorCheck {
                        category: "Transfers",
                        name: "Orphaned Transfer Jobs",
                        severity: Severity::Warning,
                        details: format!("{} transfer job(s) marked 'running' while daemon is offline", stuck),
                        fixable: true,
                    });

                    if args.repair {
                        let should_fix = args.yes || prompt_confirm(&format!("Mark {} orphaned transfers as failed?", stuck), false).unwrap_or(false);
                        if should_fix && !args.dry_run {
                            let _ = crate::services::TransferService::repair_stuck_transfers(&pool, false).await;
                            repairs_applied.push(format!("Marked {} orphaned transfers as failed", stuck));
                        }
                    }
                } else {
                    checks.push(DoctorCheck {
                        category: "Transfers",
                        name: "Transfer Subsystem State",
                        severity: Severity::Ok,
                        details: "No orphaned or stuck transfer records detected".to_string(),
                        fixable: false,
                    });
                }
            }

            // ==========================================
            // Category 5: Storage Connection Checks
            // ==========================================
            let conn_count: Result<(i64,), _> =
                sqlx::query_as("SELECT COUNT(*) FROM connections WHERE enabled = 1")
                    .fetch_one(&pool)
                    .await;

            if let Ok((conns,)) = conn_count {
                checks.push(DoctorCheck {
                    category: "Connections",
                    name: "Active Storage Providers",
                    severity: if conns > 0 { Severity::Ok } else { Severity::Warning },
                    details: format!("{} enabled storage connection(s) configured", conns),
                    fixable: false,
                });
            }
        }
        Err(e) => {
            checks.push(DoctorCheck {
                category: "Database",
                name: "SQLite Connection",
                severity: Severity::Critical,
                details: format!("Cannot connect to database: {}", e),
                fixable: false,
            });
        }
    }

    // ==========================================
    // Category 3: Storage Root & Filesystem
    // ==========================================
    let root = &ctx.config.filesystem.default_local_root;
    if root.exists() {
        let free_space = crate::api::files::get_available_disk_space(root);
        let space_str = free_space
            .map(|b| format!("({:.2} GB free)", b as f64 / (1024.0 * 1024.0 * 1024.0)))
            .unwrap_or_default();

        let severity = if let Some(bytes) = free_space {
            if bytes < 100 * 1024 * 1024 {
                Severity::Critical
            } else if bytes < 1024 * 1024 * 1024 {
                Severity::Warning
            } else {
                Severity::Ok
            }
        } else {
            Severity::Ok
        };

        checks.push(DoctorCheck {
            category: "Storage",
            name: "Local Storage Root Directory",
            severity,
            details: format!("Directory exists at {} {}", root.display(), space_str),
            fixable: false,
        });
    } else {
        checks.push(DoctorCheck {
            category: "Storage",
            name: "Local Storage Root Directory",
            severity: Severity::Warning,
            details: format!("Root directory does not exist: {}", root.display()),
            fixable: true,
        });

        if args.repair {
            let should_fix = args.yes || prompt_confirm(&format!("Create missing directory '{}'?", root.display()), true).unwrap_or(false);
            if should_fix && !args.dry_run {
                if let Ok(_) = std::fs::create_dir_all(root) {
                    repairs_applied.push(format!("Created storage root directory: {}", root.display()));
                }
            }
        }
    }

    // Temp directory
    if let Some(ref temp) = ctx.config.filesystem.temp_dir {
        if !temp.exists() {
            checks.push(DoctorCheck {
                category: "Storage",
                name: "Temporary Upload Directory",
                severity: Severity::Warning,
                details: format!("Temp directory does not exist: {}", temp.display()),
                fixable: true,
            });

            if args.repair {
                let should_fix = args.yes || prompt_confirm(&format!("Create missing temp directory '{}'?", temp.display()), true).unwrap_or(false);
                if should_fix && !args.dry_run {
                    if let Ok(_) = std::fs::create_dir_all(temp) {
                        repairs_applied.push(format!("Created temp directory: {}", temp.display()));
                    }
                }
            }
        } else {
            checks.push(DoctorCheck {
                category: "Storage",
                name: "Temporary Upload Directory",
                severity: Severity::Ok,
                details: format!("Directory exists at {}", temp.display()),
                fixable: false,
            });
        }
    }

    // ==========================================
    // Category 6: Security & Policy
    // ==========================================
    let secret = &ctx.config.security.session_secret;
    if secret == "dev_secret_change_in_production_32_chars_min" {
        checks.push(DoctorCheck {
            category: "Security",
            name: "Session Secret HMAC",
            severity: Severity::Warning,
            details: "Using default development secret. Set AEROFS_SESSION_SECRET for production.".to_string(),
            fixable: false,
        });
    } else if secret.len() < 32 {
        checks.push(DoctorCheck {
            category: "Security",
            name: "Session Secret HMAC",
            severity: Severity::Warning,
            details: "Session secret is shorter than recommended 32 characters.".to_string(),
            fixable: false,
        });
    } else {
        checks.push(DoctorCheck {
            category: "Security",
            name: "Session Secret HMAC",
            severity: Severity::Ok,
            details: "Strong cryptographic secret configured (>= 32 chars)".to_string(),
            fixable: false,
        });
    }

    if ctx.config.security.allow_symlinks_outside_root {
        checks.push(DoctorCheck {
            category: "Security",
            name: "Symlink Traversal Policy",
            severity: Severity::Warning,
            details: "allow_symlinks_outside_root is enabled (may expose host filesystem)".to_string(),
            fixable: false,
        });
    } else {
        checks.push(DoctorCheck {
            category: "Security",
            name: "Symlink Traversal Policy",
            severity: Severity::Ok,
            details: "Symlink resolution strictly sandboxed inside storage root".to_string(),
            fixable: false,
        });
    }

    // Calculate totals
    let ok_count = checks.iter().filter(|c| c.severity == Severity::Ok).count();
    let warning_count = checks.iter().filter(|c| c.severity == Severity::Warning).count();
    let critical_count = checks.iter().filter(|c| c.severity == Severity::Critical).count();

    let overall_status = if critical_count > 0 {
        "critical_issues_detected"
    } else if warning_count > 0 {
        "warnings_detected"
    } else {
        "healthy"
    };

    let report = DoctorReport {
        overall_status,
        ok_count,
        warning_count,
        critical_count,
        checks: checks.clone(),
        repairs_applied: repairs_applied.clone(),
    };

    let human_report = || {
        println!("\n🩺 AeroFS Self-Diagnostics Doctor Report:\n");
        let mut current_cat = "";
        for check in &checks {
            if check.category != current_cat {
                current_cat = check.category;
                println!("  [{}]", current_cat);
            }
            println!("    {} {}: {}", check.severity.symbol(), check.name, check.details);
        }

        println!("\nSummary: {} passed, {} warning(s), {} critical failure(s)", ok_count, warning_count, critical_count);
        if !repairs_applied.is_empty() {
            println!("\nRepairs Applied:");
            for r in &repairs_applied {
                println!("  ✓ {}", r);
            }
        }
    };

    if critical_count > 0 {
        let err = CliError::health(format!(
            "Doctor detected {} critical failure(s)",
            critical_count
        ));
        ctx.output.print_failure("doctor", &report, &err, human_report);
        Err(err)
    } else {
        ctx.output.print_success("doctor", &report, human_report);
        Ok(())
    }
}
