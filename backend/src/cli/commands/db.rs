use crate::cli::args::{DbAction, DbCommand};
use crate::cli::context::CliContext;
use crate::cli::error::CliError;
use crate::db::{backup_db, check_integrity, checkpoint_db, get_db_stats, migrate_db, vacuum_db};
use serde::Serialize;

#[derive(Serialize)]
struct DbBackupOutput {
    backup_path: String,
    status: &'static str,
}

#[derive(Serialize)]
struct DbActionOutput {
    status: &'static str,
    message: String,
}

#[derive(Serialize)]
struct DbIntegrityOutput {
    reports: Vec<String>,
    healthy: bool,
}

#[derive(Serialize)]
struct DbMigrateOutput {
    applied_migrations: Vec<String>,
    status: &'static str,
}

pub async fn handle(cmd: DbCommand, ctx: &CliContext) -> Result<(), CliError> {
    let pool = ctx.db().await?;

    match cmd.action {
        DbAction::Status => {
            let stats = get_db_stats(&pool, &ctx.config.database.url)
                .await
                .map_err(|e| CliError::database(format!("Failed to query DB statistics: {}", e)))?;

            ctx.output.print_success("db.status", &stats, || {
                println!("Database Engine Status:");
                println!("  • Location:       {}", stats.sanitized_url);
                println!("  • Journal Mode:   {}", stats.journal_mode);
                println!(
                    "  • Foreign Keys:   {}",
                    if stats.foreign_keys {
                        "Enabled (1)"
                    } else {
                        "Disabled (0)"
                    }
                );
                println!(
                    "  • Total Size:     {} bytes ({} KB)",
                    stats.total_size_bytes,
                    stats.total_size_bytes / 1024
                );
                println!("  • Users:          {}", stats.users_count);
                println!("  • Connections:    {}", stats.connections_count);
                println!("  • Transfer Jobs:  {}", stats.transfer_jobs_count);
            });
            Ok(())
        }
        DbAction::Migrate => {
            let applied = migrate_db(&pool)
                .await
                .map_err(|e| CliError::database(format!("Migration failed: {}", e)))?;

            let out = DbMigrateOutput {
                applied_migrations: applied.clone(),
                status: "success",
            };

            ctx.output.print_success("db.migrate", &out, || {
                println!(
                    "✓ Database migrations up to date ({} migrations applied):",
                    applied.len()
                );
                for m in &applied {
                    println!("  • {}", m);
                }
            });
            Ok(())
        }
        DbAction::Integrity => {
            let reports = check_integrity(&pool)
                .await
                .map_err(|e| CliError::database(format!("Integrity check error: {}", e)))?;

            let healthy = reports.iter().all(|r| r.contains("ok"));
            let out = DbIntegrityOutput {
                reports: reports.clone(),
                healthy,
            };

            let human_integrity = || {
                println!("SQLite PRAGMA integrity check results:");
                for r in &reports {
                    println!("  • {}", r);
                }
            };

            if healthy {
                ctx.output
                    .print_success("db.integrity", &out, human_integrity);
                Ok(())
            } else {
                let err = CliError::database(format!(
                    "Integrity violations detected: {}",
                    reports.join(", ")
                ));
                ctx.output
                    .print_failure("db.integrity", &out, &err, human_integrity);
                Err(err)
            }
        }
        DbAction::Vacuum => {
            vacuum_db(&pool)
                .await
                .map_err(|e| CliError::database(format!("VACUUM failed: {}", e)))?;

            let out = DbActionOutput {
                status: "ok",
                message:
                    "VACUUM completed successfully. Database defragmented and disk space reclaimed."
                        .to_string(),
            };

            ctx.output.print_success("db.vacuum", &out, || {
                println!("✓ Database VACUUM completed. Disk space reclaimed.");
            });
            Ok(())
        }
        DbAction::Checkpoint => {
            checkpoint_db(&pool)
                .await
                .map_err(|e| CliError::database(format!("WAL checkpoint failed: {}", e)))?;

            let out = DbActionOutput {
                status: "ok",
                message: "WAL journal flushed and truncated to database file.".to_string(),
            };

            ctx.output.print_success("db.checkpoint", &out, || {
                println!("✓ SQLite WAL checkpoint (TRUNCATE) completed successfully.");
            });
            Ok(())
        }
        DbAction::Backup { target, force } => {
            backup_db(&pool, &target, force)
                .await
                .map_err(|e| CliError::database(format!("Online database backup failed: {}", e)))?;

            let out = DbBackupOutput {
                backup_path: target.display().to_string(),
                status: "ok",
            };

            ctx.output.print_success("db.backup", &out, || {
                println!(
                    "✓ Online database backup snapshot created at: {}",
                    target.display()
                );
            });
            Ok(())
        }
    }
}
