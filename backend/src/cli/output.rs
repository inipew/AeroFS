use crate::cli::error::{CliError, JsonErrorDetail};
use serde::Serialize;
use std::io::{self, BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Human,
    Json,
    Quiet,
    Verbose,
}

#[derive(Debug, Serialize)]
pub struct JsonEnvelope<T: Serialize> {
    pub ok: bool,
    pub command: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonErrorDetail>,
}

#[derive(Clone)]
pub struct OutputFormatter {
    pub mode: OutputMode,
    envelope_emitted: Arc<AtomicBool>,
}

impl OutputFormatter {
    pub fn new(json: bool, quiet: bool, verbose: bool) -> Self {
        let mode = if json {
            OutputMode::Json
        } else if quiet {
            OutputMode::Quiet
        } else if verbose {
            OutputMode::Verbose
        } else {
            OutputMode::Human
        };

        Self {
            mode,
            envelope_emitted: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_json(&self) -> bool {
        self.mode == OutputMode::Json
    }

    pub fn is_quiet(&self) -> bool {
        self.mode == OutputMode::Quiet
    }

    pub fn has_emitted(&self) -> bool {
        self.envelope_emitted.load(Ordering::SeqCst)
    }

    pub fn print_success<T: Serialize>(
        &self,
        command: &'static str,
        data: &T,
        human_fn: impl FnOnce(),
    ) {
        match self.mode {
            OutputMode::Json => {
                if !self.envelope_emitted.swap(true, Ordering::SeqCst) {
                    let envelope = JsonEnvelope {
                        ok: true,
                        command,
                        data: Some(data),
                        error: None,
                    };
                    if let Ok(json_str) = serde_json::to_string_pretty(&envelope) {
                        println!("{}", json_str);
                    }
                }
            }
            OutputMode::Quiet => {
                // In quiet mode, suppress all non-essential output
            }
            OutputMode::Human | OutputMode::Verbose => {
                human_fn();
            }
        }
    }

    pub fn print_failure<T: Serialize>(
        &self,
        command: &'static str,
        data: &T,
        err: &CliError,
        human_fn: impl FnOnce(),
    ) {
        match self.mode {
            OutputMode::Json => {
                if !self.envelope_emitted.swap(true, Ordering::SeqCst) {
                    let envelope = JsonEnvelope {
                        ok: false,
                        command,
                        data: Some(data),
                        error: Some(JsonErrorDetail {
                            code: err.error_code.clone(),
                            message: err.message.clone(),
                            details: None,
                        }),
                    };
                    if let Ok(json_str) = serde_json::to_string_pretty(&envelope) {
                        println!("{}", json_str);
                    }
                }
            }
            OutputMode::Quiet => {}
            OutputMode::Human | OutputMode::Verbose => {
                human_fn();
                eprintln!("❌ Error: {}", err.message);
            }
        }
    }

    pub fn print_error(&self, command: &'static str, err: &CliError) {
        match self.mode {
            OutputMode::Json => {
                if !self.envelope_emitted.swap(true, Ordering::SeqCst) {
                    let envelope = JsonEnvelope::<serde_json::Value> {
                        ok: false,
                        command,
                        data: None,
                        error: Some(JsonErrorDetail {
                            code: err.error_code.clone(),
                            message: err.message.clone(),
                            details: None,
                        }),
                    };
                    if let Ok(json_str) = serde_json::to_string_pretty(&envelope) {
                        eprintln!("{}", json_str);
                    }
                }
            }
            OutputMode::Quiet | OutputMode::Human | OutputMode::Verbose => {
                eprintln!("❌ Error: {}", err.message);
            }
        }
    }
}

#[cfg(unix)]
struct TermiosGuard {
    fd: std::os::unix::io::RawFd,
    orig: libc::termios,
}

#[cfg(unix)]
impl Drop for TermiosGuard {
    fn drop(&mut self) {
        unsafe { libc::tcsetattr(self.fd, libc::TCSANOW, &self.orig) };
    }
}

/// Safely prompt for a password from terminal with echo disabled
pub fn read_password_prompt(prompt: &str) -> anyhow::Result<String> {
    print!("{}", prompt);
    io::stdout().flush()?;

    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        let fd = io::stdin().as_raw_fd();
        let mut termios: libc::termios = unsafe { std::mem::zeroed() };
        let ret = unsafe { libc::tcgetattr(fd, &mut termios) };
        if ret == 0 {
            let guard = TermiosGuard { fd, orig: termios };

            let mut silent_termios = termios;
            silent_termios.c_lflag &= !(libc::ECHO | libc::ECHONL);
            unsafe { libc::tcsetattr(fd, libc::TCSANOW, &silent_termios) };

            let mut pass = String::new();
            let read_res = io::stdin().read_line(&mut pass);

            drop(guard); // Explicit restore before newline
            println!(); // Print newline since ECHO was off

            read_res?;
            return Ok(pass.trim_end_matches(&['\r', '\n'][..]).to_string());
        }
    }

    // Fallback if not unix or tcgetattr failed
    let mut pass = String::new();
    io::stdin().read_line(&mut pass)?;
    Ok(pass.trim_end_matches(&['\r', '\n'][..]).to_string())
}

/// Read password from stdin pipe for scripted automation
pub fn read_password_stdin() -> anyhow::Result<String> {
    let mut pass = String::new();
    io::stdin().read_line(&mut pass)?;
    Ok(pass.trim_end_matches(&['\r', '\n'][..]).to_string())
}

/// Interactively prompt for confirmation (y/N)
pub fn prompt_confirm(prompt: &str, default_yes: bool) -> anyhow::Result<bool> {
    let hint = if default_yes { "[Y/n]" } else { "[y/N]" };
    print!("{} {} ", prompt, hint);
    io::stdout().flush()?;

    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    let trimmed = line.trim().to_lowercase();

    if trimmed.is_empty() {
        return Ok(default_yes);
    }

    Ok(trimmed == "y" || trimmed == "yes")
}
