//! Typed application events (§121) — first-class abstraction over informal DomainEvent.
//! DomainEvent remains wire/journal format; AppEvent is typed application layer.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::DomainEvent;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", content = "payload")]
pub enum FileEvent {
    Created {
        connection_id: String,
        path: String,
    },
    Deleted {
        connection_id: String,
        path: String,
    },
    Moved {
        connection_id: String,
        from: String,
        to: String,
    },
    Updated {
        connection_id: String,
        path: String,
    },
    Renamed {
        connection_id: String,
        from: String,
        to: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", content = "payload")]
pub enum TransferEvent {
    Progress(serde_json::Value),
    Completed(serde_json::Value),
    Failed(serde_json::Value),
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", content = "payload")]
pub enum ConnectionEvent {
    Created {
        connection_id: String,
    },
    Updated {
        connection_id: String,
    },
    Deleted {
        connection_id: String,
    },
    StatusChanged {
        connection_id: String,
        status: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", content = "payload")]
pub enum AuthEvent {
    Login { user_id: String },
    Logout { user_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", content = "payload")]
pub enum ShareEvent {
    Created { share_id: String },
    Deleted { share_id: String },
}

/// Top-level application event — single source for cross-cutting side effects.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", content = "data")]
pub enum AppEvent {
    File(FileEvent),
    Transfer(TransferEvent),
    Connection(ConnectionEvent),
    Auth(AuthEvent),
    Share(ShareEvent),
}

impl From<AppEvent> for DomainEvent {
    fn from(ev: AppEvent) -> Self {
        match ev {
            AppEvent::File(f) => match f {
                FileEvent::Created {
                    connection_id,
                    path,
                }
                | FileEvent::Updated {
                    connection_id,
                    path,
                } => DomainEvent::FileChange {
                    connection_id,
                    path: path.clone(),
                    action: "create".into(),
                    old_path: None,
                    parent_path: Some(
                        std::path::Path::new(&path)
                            .parent()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|| "/".into()),
                    ),
                    old_parent_path: None,
                },
                FileEvent::Deleted {
                    connection_id,
                    path,
                } => DomainEvent::FileChange {
                    connection_id,
                    path: path.clone(),
                    action: "delete".into(),
                    old_path: None,
                    parent_path: None,
                    old_parent_path: None,
                },
                FileEvent::Moved {
                    connection_id,
                    from,
                    to,
                } => DomainEvent::file_rename(connection_id, from, to),
                FileEvent::Renamed {
                    connection_id,
                    from,
                    to,
                } => DomainEvent::file_rename(connection_id, from, to),
            },
            AppEvent::Transfer(t) => match t {
                TransferEvent::Progress(v) => DomainEvent::TransferProgress(v),
                TransferEvent::Completed(v) => DomainEvent::TransferCompleted(v),
                TransferEvent::Failed(v) => DomainEvent::TransferFailed(v),
            },
            AppEvent::Connection(c) => match c {
                ConnectionEvent::StatusChanged { connection_id, .. } => {
                    DomainEvent::PermissionChanged {
                        user_id: "system".into(),
                        connection_id: Some(connection_id),
                    }
                }
                ConnectionEvent::Created { connection_id }
                | ConnectionEvent::Updated { connection_id }
                | ConnectionEvent::Deleted { connection_id } => DomainEvent::PermissionChanged {
                    user_id: "system".into(),
                    connection_id: Some(connection_id),
                },
            },
            AppEvent::Auth(a) => match a {
                AuthEvent::Login { user_id } | AuthEvent::Logout { user_id } => {
                    DomainEvent::PermissionChanged {
                        user_id,
                        connection_id: None,
                    }
                }
            },
            AppEvent::Share(_) => DomainEvent::PermissionChanged {
                user_id: "system".into(),
                connection_id: None,
            },
        }
    }
}
