use crate::cli::args::{ConfigAction, ConfigCommand};
use crate::cli::context::CliContext;
use crate::cli::error::CliError;
use serde::Serialize;

#[derive(Serialize)]
struct ConfigValidationOutput {
    valid: bool,
    message: String,
}

#[derive(Serialize)]
struct ConfigGetOutput {
    key: String,
    value: String,
}

pub async fn handle(cmd: ConfigCommand, ctx: &CliContext) -> Result<(), CliError> {
    match cmd.action {
        ConfigAction::Show => {
            let mut sanitized = ctx.config.clone();
            if !sanitized.security.session_secret.is_empty() {
                sanitized.security.session_secret = "********".to_string();
            }

            ctx.output.print_success("config.show", &sanitized, || {
                println!("{}", ctx.config.to_sanitized_toml());
            });
            Ok(())
        }
        ConfigAction::Effective => {
            let provenance = ctx
                .config
                .get_effective_provenance(ctx.config_path.as_deref());
            ctx.output
                .print_success("config.effective", &provenance, || {
                    println!("AeroFS Layered Effective Configuration:");
                    for entry in &provenance {
                        println!(
                            "  • {:<35} = {:<25} (Source: {})",
                            entry.key, entry.value, entry.source
                        );
                    }
                });
            Ok(())
        }
        ConfigAction::Get { key } => {
            if let Some(val) = ctx.config.get_by_key_path(&key) {
                let out = ConfigGetOutput {
                    key: key.clone(),
                    value: val.clone(),
                };
                ctx.output.print_success("config.get", &out, || {
                    println!("{}: {}", key, val);
                });
                Ok(())
            } else {
                Err(CliError::not_found(format!(
                    "Unknown configuration key '{}'",
                    key
                )))
            }
        }
        ConfigAction::Explain { key } => {
            if let Some(desc) = crate::config::AppConfig::describe_key(&key) {
                let effective_val = ctx
                    .config
                    .get_by_key_path(&key)
                    .unwrap_or_else(|| "N/A".to_string());
                ctx.output.print_success("config.explain", &desc, || {
                    println!("Configuration Key: {}", desc.key);
                    println!("  • Description:      {}", desc.description);
                    println!("  • Type:             {}", desc.value_type);
                    println!("  • Default Value:    {}", desc.default_value);
                    println!("  • Effective Value:  {}", effective_val);
                    if let Some(env) = desc.env_variable {
                        println!("  • Environment Var:  {}", env);
                    }
                    println!(
                        "  • Runtime Mutable:  {}",
                        if desc.runtime_mutable { "Yes" } else { "No" }
                    );
                    println!(
                        "  • Restart Required: {}",
                        if desc.restart_required { "Yes" } else { "No" }
                    );
                    println!("  • Subsystems:       {}", desc.subsystems.join(", "));
                });
                Ok(())
            } else {
                Err(CliError::not_found(format!(
                    "No schema metadata found for configuration key '{}'",
                    key
                )))
            }
        }
        ConfigAction::Validate => match ctx.config.validate() {
            Ok(_) => {
                let out = ConfigValidationOutput {
                    valid: true,
                    message: "Configuration is valid and passes all consistency checks".to_string(),
                };
                ctx.output.print_success("config.validate", &out, || {
                    println!("✓ Configuration is valid and passes all checks.");
                });
                Ok(())
            }
            Err(e) => Err(CliError::config(format!(
                "Configuration validation failed: {}",
                e
            ))),
        },
    }
}
