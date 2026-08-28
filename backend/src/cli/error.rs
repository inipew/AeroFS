use serde::Serialize;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
    Success = 0,
    GeneralError = 1,
    InvalidUsage = 2,
    ConfigError = 3,
    DatabaseError = 4,
    DaemonNotRunning = 5,
    DaemonAlreadyRunning = 6,
    HealthCheckFailed = 7,
    PermissionDenied = 8,
    NotFound = 9,
    Conflict = 10,
}

impl ExitCode {
    pub fn as_i32(&self) -> i32 {
        *self as i32
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonErrorDetail {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

#[derive(Debug)]
pub struct CliError {
    pub code: ExitCode,
    pub message: String,
    pub error_code: String,
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CliError {}

impl CliError {
    pub fn new(code: ExitCode, error_code: &str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            error_code: error_code.to_string(),
        }
    }

    pub fn usage(message: impl Into<String>) -> Self {
        Self::new(ExitCode::InvalidUsage, "INVALID_USAGE", message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ExitCode::NotFound, "NOT_FOUND", message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(ExitCode::Conflict, "CONFLICT", message)
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(ExitCode::PermissionDenied, "FORBIDDEN", message)
    }

    pub fn config(message: impl Into<String>) -> Self {
        Self::new(ExitCode::ConfigError, "CONFIG_ERROR", message)
    }

    pub fn database(message: impl Into<String>) -> Self {
        Self::new(ExitCode::DatabaseError, "DATABASE_ERROR", message)
    }

    pub fn health(message: impl Into<String>) -> Self {
        Self::new(ExitCode::HealthCheckFailed, "HEALTH_CHECK_FAILED", message)
    }

    pub fn general(message: impl Into<String>) -> Self {
        Self::new(ExitCode::GeneralError, "GENERAL_ERROR", message)
    }
}
