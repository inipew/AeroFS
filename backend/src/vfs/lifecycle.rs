use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Explicit lifecycle state machine for VFS storage providers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", content = "detail")]
pub enum ProviderState {
    #[serde(rename = "initializing")]
    Initializing,
    #[serde(rename = "connecting")]
    Connecting,
    #[serde(rename = "ready")]
    Ready,
    #[serde(rename = "degraded")]
    Degraded {
        since: DateTime<Utc>,
        reason: String,
    },
    #[serde(rename = "draining")]
    Draining, // hot-swap or shutdown: allow existing operations to finish, reject new ones
    #[serde(rename = "disconnected")]
    Disconnected,
    #[serde(rename = "failed")]
    Failed { reason: String },
}

impl ProviderState {
    pub fn is_operational(&self) -> bool {
        matches!(self, ProviderState::Ready | ProviderState::Degraded { .. })
    }

    pub fn is_ready(&self) -> bool {
        matches!(self, ProviderState::Ready)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ProviderState::Initializing => "initializing",
            ProviderState::Connecting => "connecting",
            ProviderState::Ready => "ready",
            ProviderState::Degraded { .. } => "degraded",
            ProviderState::Draining => "draining",
            ProviderState::Disconnected => "disconnected",
            ProviderState::Failed { .. } => "failed",
        }
    }
}
