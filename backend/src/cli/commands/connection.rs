use crate::auth::session::UserInfo;
use crate::auth::AuthenticatedUser;
use crate::cli::args::{ConnectionAction, ConnectionCommand};
use crate::cli::context::CliContext;
use crate::cli::error::CliError;
use crate::services::connection_service::ConnectionService;
use serde::Serialize;

#[derive(Serialize)]
struct ConnectionActionOutput {
    id: String,
    status: &'static str,
    message: String,
}

pub async fn handle(cmd: ConnectionCommand, ctx: &CliContext) -> Result<(), CliError> {
    let state = ctx.state().await?;

    let admin_user = AuthenticatedUser(UserInfo {
        id: "cli_admin".to_string(),
        username: "admin".to_string(),
        is_admin: true,
    });

    match cmd.action {
        ConnectionAction::List => {
            let connections = ConnectionService::list_connections(&state, &admin_user)
                .await
                .map_err(|e| CliError::database(format!("Failed to list connections: {}", e)))?;

            ctx.output.print_success("connection.list", &connections, || {
                if connections.is_empty() {
                    println!("No storage connections configured.");
                } else {
                    println!("Storage Connections ({} total):", connections.len());
                    for c in &connections {
                        let status_str = match c.status {
                            crate::domain::ConnectionStatus::Connected => "Connected",
                            crate::domain::ConnectionStatus::Disconnected => "Disabled",
                            crate::domain::ConnectionStatus::Connecting => "Connecting",
                            crate::domain::ConnectionStatus::Reconnecting => "Reconnecting",
                            crate::domain::ConnectionStatus::Failed => "Failed",
                        };
                        println!(
                            "  • {:<12} | {:<20} | Provider: {:<6} | Path: {:<15} | Status: {}",
                            c.id,
                            c.name,
                            c.provider.as_str(),
                            c.base_path,
                            status_str
                        );
                    }
                }
            });
            Ok(())
        }
        ConnectionAction::Show { id } => {
            let detail = ConnectionService::get_connection(&state, &admin_user, &id)
                .await
                .map_err(|e| match e {
                    crate::errors::AppError::NotFound(_) | crate::errors::AppError::Vfs(_) => {
                        CliError::not_found(format!("Connection '{}' not found", id))
                    }
                    _ => CliError::database(format!("Failed to get connection: {}", e)),
                })?;

            ctx.output.print_success("connection.show", &detail, || {
                println!("Connection Details:");
                println!("  • ID:                  {}", detail.connection.id);
                println!("  • Name:                {}", detail.connection.name);
                println!("  • Provider:            {}", detail.connection.provider.as_str());
                if let Some(host) = &detail.connection.host {
                    println!("  • Host:                {}:{}", host, detail.connection.port.unwrap_or(0));
                }
                if let Some(user) = &detail.connection.username {
                    println!("  • Username:            {}", user);
                }
                println!("  • Base Path:           {}", detail.connection.base_path);
                println!("  • Read-Only:           {}", if detail.connection.read_only { "Yes" } else { "No" });
                println!("  • Enabled:             {}", if detail.connection.enabled { "Yes" } else { "No" });
                println!("Capabilities:");
                println!("  • Read / Download:     {}", if detail.capabilities.read { "Yes" } else { "No" });
                println!("  • Write / Upload:      {}", if detail.capabilities.write { "Yes" } else { "No" });
                println!("  • Atomic Write:        {}", if detail.capabilities.atomic_write { "Yes" } else { "No" });
                println!("  • Server-Side Copy:    {}", if detail.capabilities.server_side_copy { "Yes" } else { "No" });
                println!("  • Checksum & Integrity:{}", if detail.capabilities.checksum { "Yes" } else { "No" });
                println!("  • Symlink Resolution:  {}", if detail.capabilities.symlink { "Yes" } else { "No" });
            });
            Ok(())
        }
        ConnectionAction::Test { id } => {
            let res = ConnectionService::test_connection(&state, &admin_user, &id)
                .await
                .map_err(|e| CliError::health(format!("Connection test failed: {}", e)))?;

            let human_test = || {
                println!("Connection Diagnostic Test for '{}':", id);
                println!("  • Reachable:           {}", if res.success { "Yes" } else { "No" });
                println!("  • Latency:             {} ms", res.latency_ms);
                println!("  • Message:             {}", res.message);
            };

            if res.success {
                ctx.output.print_success("connection.test", &res, human_test);
                Ok(())
            } else {
                let err = CliError::health(format!("Connection test for '{}' failed: {}", id, res.message));
                ctx.output.print_failure("connection.test", &res, &err, human_test);
                Err(err)
            }
        }
        ConnectionAction::Enable { id } => {
            let pool = ctx.db().await?;
            let now = chrono::Utc::now().to_rfc3339();
            let res = sqlx::query("UPDATE connections SET enabled = 1, updated_at = ? WHERE id = ?")
                .bind(&now)
                .bind(&id)
                .execute(&pool)
                .await
                .map_err(|e| CliError::database(format!("DB error: {}", e)))?;

            if res.rows_affected() == 0 {
                return Err(CliError::not_found(format!("Connection '{}' not found", id)));
            }

            let out = ConnectionActionOutput {
                id: id.clone(),
                status: "enabled",
                message: format!("Connection '{}' enabled successfully", id),
            };

            ctx.output.print_success("connection.enable", &out, || {
                println!("✓ Storage connection '{}' enabled.", id);
            });
            Ok(())
        }
        ConnectionAction::Disable { id } => {
            if id == "local" {
                return Err(CliError::forbidden("Default Local connection cannot be disabled"));
            }

            let pool = ctx.db().await?;
            let now = chrono::Utc::now().to_rfc3339();
            let res = sqlx::query("UPDATE connections SET enabled = 0, updated_at = ? WHERE id = ?")
                .bind(&now)
                .bind(&id)
                .execute(&pool)
                .await
                .map_err(|e| CliError::database(format!("DB error: {}", e)))?;

            if res.rows_affected() == 0 {
                return Err(CliError::not_found(format!("Connection '{}' not found", id)));
            }

            let out = ConnectionActionOutput {
                id: id.clone(),
                status: "disabled",
                message: format!("Connection '{}' disabled successfully", id),
            };

            ctx.output.print_success("connection.disable", &out, || {
                println!("✓ Storage connection '{}' disabled.", id);
            });
            Ok(())
        }
    }
}
