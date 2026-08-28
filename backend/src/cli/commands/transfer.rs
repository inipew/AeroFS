use crate::cli::args::{TransferAction, TransferCommand};
use crate::cli::context::CliContext;
use crate::cli::error::CliError;
use crate::cli::output::prompt_confirm;
use crate::services::TransferService;
use serde::Serialize;

#[derive(Serialize)]
struct TransferPurgeOutput {
    purged_records: usize,
    days_cutoff: u32,
    dry_run: bool,
}

#[derive(Serialize)]
struct TransferRepairOutput {
    repaired_records: usize,
    dry_run: bool,
}

#[derive(Serialize)]
struct TransferActionOutput {
    id: String,
    status: &'static str,
    message: String,
}

pub async fn handle(cmd: TransferCommand, ctx: &CliContext) -> Result<(), CliError> {
    let pool = ctx.db().await?;

    match cmd.action {
        TransferAction::List {
            status,
            limit,
            user,
            connection,
        } => {
            let jobs = TransferService::list_transfers_filtered(
                &pool,
                status.as_deref(),
                limit,
                user.as_deref(),
                connection.as_deref(),
            )
            .await
            .map_err(|e| CliError::database(format!("Failed to list transfers: {}", e)))?;

            ctx.output.print_success("transfer.list", &jobs, || {
                if jobs.is_empty() {
                    println!("No transfer jobs found matching criteria.");
                } else {
                    println!("Transfer Jobs ({} records):", jobs.len());
                    for j in &jobs {
                        let id_short = if j.id.len() >= 8 { &j.id[..8] } else { &j.id };
                        println!(
                            "  [{}] {:<12} | {:<4} | {}:{} -> {}:{} ({}/{} bytes)",
                            id_short,
                            j.status.as_str(),
                            j.transfer_type.as_str(),
                            j.source_connection_id,
                            j.source_path,
                            j.destination_connection_id,
                            j.destination_path,
                            j.transferred_bytes,
                            j.total_bytes,
                        );
                    }
                }
            });
            Ok(())
        }
        TransferAction::Show { id } => {
            let job = TransferService::get_transfer(&pool, &id)
                .await
                .map_err(|e| CliError::database(format!("Failed to retrieve transfer: {}", e)))?;

            if let Some(j) = job {
                ctx.output.print_success("transfer.show", &j, || {
                    println!("Transfer Job Details:");
                    println!("  • ID:                  {}", j.id);
                    println!("  • Name:                {}", j.name);
                    println!("  • Type:                {}", j.transfer_type.as_str());
                    println!("  • Status:              {}", j.status.as_str());
                    println!("  • Phase:               {}", j.phase.as_str());
                    println!(
                        "  • User ID:             {}",
                        j.user_id.as_deref().unwrap_or("System")
                    );
                    println!(
                        "  • Source:              {}:{}",
                        j.source_connection_id, j.source_path
                    );
                    println!(
                        "  • Destination:         {}:{}",
                        j.destination_connection_id, j.destination_path
                    );
                    println!(
                        "  • Progress:            {} / {} bytes ({:.1}%)",
                        j.transferred_bytes,
                        j.total_bytes,
                        if j.total_bytes > 0 {
                            (j.transferred_bytes as f64 / j.total_bytes as f64) * 100.0
                        } else {
                            0.0
                        }
                    );
                    println!(
                        "  • Speed:               {} bytes/sec",
                        j.speed_bytes_per_sec
                    );
                    if let Some(eta) = j.eta_seconds {
                        println!("  • ETA:                 {} seconds", eta);
                    }
                    if let Some(err) = &j.error_message {
                        println!("  • Error:               {}", err);
                    }
                    println!("  • Created At:          {}", j.created_at);
                    println!("  • Updated At:          {}", j.updated_at);
                });
                Ok(())
            } else {
                Err(CliError::not_found(format!(
                    "Transfer job '{}' not found",
                    id
                )))
            }
        }
        TransferAction::Cancel { id } => {
            // Attempt cancellation via AppState & TransferManager
            let state = ctx.state().await?;
            let cancelled = state
                .transfer_manager
                .cancel_job(&id, None, true)
                .await
                .unwrap_or(false);

            if cancelled {
                let out = TransferActionOutput {
                    id: id.clone(),
                    status: "cancelled",
                    message: format!("Transfer job '{}' successfully cancelled", id),
                };
                ctx.output.print_success("transfer.cancel", &out, || {
                    println!("✓ Transfer job '{}' cancelled.", id);
                });
                Ok(())
            } else {
                Err(CliError::not_found(format!(
                    "Transfer job '{}' is not running or does not exist",
                    id
                )))
            }
        }
        TransferAction::Dismiss { id } => {
            let state = ctx.state().await?;
            let dismissed = state
                .transfer_manager
                .dismiss_job(&id, None, true)
                .await
                .unwrap_or(false);

            if dismissed {
                let out = TransferActionOutput {
                    id: id.clone(),
                    status: "dismissed",
                    message: format!("Transfer job '{}' dismissed from history", id),
                };
                ctx.output.print_success("transfer.dismiss", &out, || {
                    println!("✓ Transfer job '{}' dismissed.", id);
                });
                Ok(())
            } else {
                Err(CliError::not_found(format!(
                    "Transfer job '{}' not found",
                    id
                )))
            }
        }
        TransferAction::Clear => {
            let state = ctx.state().await?;
            let cleared = state
                .transfer_manager
                .clear_finished_jobs(None, true)
                .await
                .map_err(|e| {
                    CliError::general(format!("Failed to clear finished transfers: {}", e))
                })?;

            let out = serde_json::json!({
                "cleared_count": cleared,
                "status": "ok"
            });

            ctx.output.print_success("transfer.clear", &out, || {
                println!(
                    "✓ Cleared {} finished transfer job(s) from history.",
                    cleared
                );
            });
            Ok(())
        }
        TransferAction::Purge { days, dry_run, yes } => {
            if days == 0 {
                return Err(CliError::usage("Purge days must be at least 1 day"));
            }

            if dry_run {
                let count = TransferService::purge_transfers_older_than(&pool, days, true)
                    .await
                    .map_err(|e| CliError::database(format!("Dry-run query error: {}", e)))?;

                let out = TransferPurgeOutput {
                    purged_records: count,
                    days_cutoff: days,
                    dry_run: true,
                };
                ctx.output.print_success("transfer.purge", &out, || {
                    println!(
                        "ℹ Dry-run: Would purge {} transfer record(s) older than {} days.",
                        count, days
                    );
                });
                Ok(())
            } else {
                let confirmed = yes
                    || prompt_confirm(
                        &format!("Purge finished transfer records older than {} days?", days),
                        false,
                    )
                    .unwrap_or(false);

                if !confirmed {
                    println!("Purge cancelled by user.");
                    return Ok(());
                }

                let count = TransferService::purge_transfers_older_than(&pool, days, false)
                    .await
                    .map_err(|e| CliError::database(format!("Purge execution error: {}", e)))?;

                let out = TransferPurgeOutput {
                    purged_records: count,
                    days_cutoff: days,
                    dry_run: false,
                };
                ctx.output.print_success("transfer.purge", &out, || {
                    println!(
                        "✓ Successfully purged {} transfer record(s) older than {} days.",
                        count, days
                    );
                });
                Ok(())
            }
        }
        TransferAction::Repair { dry_run, yes } => {
            if dry_run {
                let count = TransferService::repair_stuck_transfers(&pool, true)
                    .await
                    .map_err(|e| CliError::database(format!("Query error: {}", e)))?;

                let out = TransferRepairOutput {
                    repaired_records: count,
                    dry_run: true,
                };
                ctx.output.print_success("transfer.repair", &out, || {
                    println!(
                        "ℹ Dry-run: Found {} stuck transfer record(s) that would be marked failed.",
                        count
                    );
                });
                Ok(())
            } else {
                let confirmed = yes
                    || prompt_confirm(
                        "Mark all orphaned/stuck running transfers as failed?",
                        false,
                    )
                    .unwrap_or(false);

                if !confirmed {
                    println!("Repair cancelled by user.");
                    return Ok(());
                }

                let count = TransferService::repair_stuck_transfers(&pool, false)
                    .await
                    .map_err(|e| CliError::database(format!("Repair error: {}", e)))?;

                let out = TransferRepairOutput {
                    repaired_records: count,
                    dry_run: false,
                };
                ctx.output.print_success("transfer.repair", &out, || {
                    println!("✓ Repaired {} stuck transfer record(s).", count);
                });
                Ok(())
            }
        }
    }
}
