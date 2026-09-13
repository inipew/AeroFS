use crate::bootstrap::build_user_service;
use crate::cli::args::{UserAction, UserCommand};
use crate::cli::context::CliContext;
use crate::cli::error::CliError;
use crate::cli::output::{prompt_confirm, read_password_prompt, read_password_stdin};
use serde::Serialize;

#[derive(Serialize)]
struct UserActionOutput {
    username: String,
    status: &'static str,
    message: String,
}

pub async fn handle(cmd: UserCommand, ctx: &CliContext) -> Result<(), CliError> {
    let pool = ctx.db().await?;
    let users = build_user_service(pool);

    match cmd.action {
        UserAction::List => {
            let user_list = users
                .list_users()
                .await
                .map_err(|e| CliError::database(format!("Failed to query users: {}", e)))?;

            ctx.output.print_success("user.list", &user_list, || {
                println!("Registered System Users ({} total):", user_list.len());
                for u in &user_list {
                    let role = if u.is_admin {
                        "Administrator"
                    } else {
                        "Standard User"
                    };
                    println!(
                        "  • {:<20} [{:<13}] (Created: {})",
                        u.username, role, u.created_at
                    );
                }
            });
            Ok(())
        }
        UserAction::Show { username } => {
            let user = users.get_user(&username).await.map_err(|e| match e {
                crate::errors::AppError::NotFound(_) => {
                    CliError::not_found(format!("User '{}' not found", username))
                }
                _ => CliError::database(format!("Database error: {}", e)),
            })?;

            ctx.output.print_success("user.show", &user, || {
                println!("User Account Details:");
                println!("  • ID:                  {}", user.id);
                println!("  • Username:            {}", user.username);
                println!(
                    "  • Role:                {}",
                    if user.is_admin {
                        "Administrator"
                    } else {
                        "Standard User"
                    }
                );
                println!(
                    "  • Custom Permissions:  {} granted",
                    user.permissions_count
                );
                println!("  • Created At:          {}", user.created_at);
                println!("  • Updated At:          {}", user.updated_at);
            });
            Ok(())
        }
        UserAction::Create {
            username,
            admin,
            password_stdin,
        } => {
            let password = if password_stdin {
                read_password_stdin().map_err(|e| {
                    CliError::usage(format!("Failed to read password from stdin: {}", e))
                })?
            } else {
                let p1 = read_password_prompt(&format!("Enter password for user '{}': ", username))
                    .map_err(|e| CliError::usage(format!("Failed to read password: {}", e)))?;
                let p2 = read_password_prompt("Confirm password: ").map_err(|e| {
                    CliError::usage(format!("Failed to read password confirmation: {}", e))
                })?;
                if p1 != p2 {
                    return Err(CliError::usage("Passwords do not match"));
                }
                p1
            };

            let uid = users
                .create_user(&username, &password, admin)
                .await
                .map_err(|e| match e {
                    crate::errors::AppError::Conflict(msg) => CliError::conflict(msg),
                    crate::errors::AppError::BadRequest(msg) => CliError::usage(msg),
                    _ => CliError::database(format!("Failed to create user: {}", e)),
                })?;

            let out = UserActionOutput {
                username: username.clone(),
                status: "created",
                message: format!("User '{}' (ID: {}) created successfully", username, uid),
            };

            ctx.output.print_success("user.create", &out, || {
                println!(
                    "✓ User '{}' created successfully (Admin: {}).",
                    username, admin
                );
            });
            Ok(())
        }
        UserAction::Passwd {
            username,
            password_stdin,
        } => {
            users
                .get_user(&username)
                .await
                .map_err(|_| CliError::not_found(format!("User '{}' not found", username)))?;

            let password = if password_stdin {
                read_password_stdin().map_err(|e| {
                    CliError::usage(format!("Failed to read password from stdin: {}", e))
                })?
            } else {
                let p1 =
                    read_password_prompt(&format!("Enter new password for user '{}': ", username))
                        .map_err(|e| CliError::usage(format!("Failed to read password: {}", e)))?;
                let p2 = read_password_prompt("Confirm new password: ").map_err(|e| {
                    CliError::usage(format!("Failed to read password confirmation: {}", e))
                })?;
                if p1 != p2 {
                    return Err(CliError::usage("Passwords do not match"));
                }
                p1
            };

            users
                .update_password(&username, &password)
                .await
                .map_err(|e| match e {
                    crate::errors::AppError::BadRequest(msg) => CliError::usage(msg),
                    crate::errors::AppError::NotFound(msg) => CliError::not_found(msg),
                    _ => CliError::database(format!("Failed to update password: {}", e)),
                })?;

            let out = UserActionOutput {
                username: username.clone(),
                status: "updated",
                message: format!("Password for user '{}' successfully updated", username),
            };

            ctx.output.print_success("user.passwd", &out, || {
                println!("✓ Password for user '{}' updated successfully.", username);
            });
            Ok(())
        }
        UserAction::Delete { username, yes } => {
            if !yes {
                let confirmed = prompt_confirm(
                    &format!(
                        "Are you sure you want to permanently delete user '{}'?",
                        username
                    ),
                    false,
                )
                .unwrap_or(false);
                if !confirmed {
                    println!("Deletion cancelled.");
                    return Ok(());
                }
            }

            users.delete_user(&username).await.map_err(|e| match e {
                crate::errors::AppError::Forbidden(msg) => CliError::forbidden(msg),
                crate::errors::AppError::NotFound(msg) => CliError::not_found(msg),
                _ => CliError::database(format!("Failed to delete user: {}", e)),
            })?;

            let out = UserActionOutput {
                username: username.clone(),
                status: "deleted",
                message: format!("User '{}' successfully deleted", username),
            };

            ctx.output.print_success("user.delete", &out, || {
                println!("✓ User '{}' deleted successfully.", username);
            });
            Ok(())
        }
        UserAction::Promote { username } => {
            users
                .set_admin_role(&username, true)
                .await
                .map_err(|e| match e {
                    crate::errors::AppError::NotFound(msg) => CliError::not_found(msg),
                    _ => CliError::database(format!("Failed to promote user: {}", e)),
                })?;

            let out = UserActionOutput {
                username: username.clone(),
                status: "promoted",
                message: format!("User '{}' promoted to administrator", username),
            };

            ctx.output.print_success("user.promote", &out, || {
                println!("✓ User '{}' is now an administrator.", username);
            });
            Ok(())
        }
        UserAction::Demote { username } => {
            users
                .set_admin_role(&username, false)
                .await
                .map_err(|e| match e {
                    crate::errors::AppError::Forbidden(msg) => CliError::forbidden(msg),
                    crate::errors::AppError::NotFound(msg) => CliError::not_found(msg),
                    _ => CliError::database(format!("Failed to demote user: {}", e)),
                })?;

            let out = UserActionOutput {
                username: username.clone(),
                status: "demoted",
                message: format!("Administrator privileges removed for user '{}'", username),
            };

            ctx.output.print_success("user.demote", &out, || {
                println!("✓ User '{}' demoted to standard user.", username);
            });
            Ok(())
        }
    }
}
