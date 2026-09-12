use crate::auth::AuthenticatedUser;
use crate::events::ReplayOutcome;
use crate::state::AppState;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::IntoResponse,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

#[derive(Debug, Deserialize)]
pub struct WsQuery {
    pub last_seq: Option<u64>,
    pub last_epoch: Option<String>,
}

pub async fn ws_handler(
    user: AuthenticatedUser,
    ws: WebSocketUpgrade,
    Query(query): Query<WsQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let shutdown_token = state.runtime.shutdown_token.clone();
    ws.on_upgrade(move |socket| {
        handle_socket(
            socket,
            state,
            user,
            query.last_epoch,
            query.last_seq,
            shutdown_token,
        )
    })
}

fn is_event_authorized(
    event: &crate::events::DomainEvent,
    user_id: &str,
    is_admin: bool,
    authorized_conns: &HashSet<String>,
) -> bool {
    if is_admin {
        return true;
    }

    match event {
        crate::events::DomainEvent::TransferProgress(val)
        | crate::events::DomainEvent::TransferCompleted(val)
        | crate::events::DomainEvent::TransferFailed(val)
        | crate::events::DomainEvent::TransferCancelled(val) => {
            // 1. Align with TransferService::authorize_transfer_visibility:
            // Job owner always has visibility to their own transfer events
            if let Some(owner) = val.get("user_id").and_then(|v| v.as_str()) {
                if owner == user_id {
                    return true;
                }
            }

            let src = val.get("source_connection_id").and_then(|v| v.as_str());
            let dst = val
                .get("destination_connection_id")
                .and_then(|v| v.as_str());

            // 2. Browser uploads have a synthetic `upload` source; visibility determined by target
            if val.get("transfer_type").and_then(|v| v.as_str()) == Some("upload") {
                return dst.is_some_and(|connection_id| authorized_conns.contains(connection_id));
            }

            // 3. Downloads have a synthetic `download` destination; visibility determined by source
            if val.get("transfer_type").and_then(|v| v.as_str()) == Some("download") {
                return src.is_some_and(|connection_id| authorized_conns.contains(connection_id));
            }

            // 4. Copy/move/sync must remain visible only when both real endpoints are authorized
            match (src, dst) {
                (Some(s), Some(d)) => authorized_conns.contains(s) && authorized_conns.contains(d),
                (Some(s), None) => authorized_conns.contains(s),
                (None, Some(d)) => authorized_conns.contains(d),
                _ => false,
            }
        }
        crate::events::DomainEvent::FileChange { connection_id, .. } => {
            authorized_conns.contains(connection_id)
        }
        crate::events::DomainEvent::ResyncRequired { .. } => true,
        crate::events::DomainEvent::PermissionChanged { .. } => true,
        crate::events::DomainEvent::FullSync { .. } => true,
    }
}

async fn reload_permissions(
    db: &crate::db::DbPool,
    user_id: &str,
    is_admin: bool,
    authorized_conns: &Arc<RwLock<HashSet<String>>>,
) {
    let mut conns = HashSet::new();
    conns.insert("local".to_string());
    if !is_admin {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT connection_id FROM permissions WHERE user_id = ? AND can_read = 1",
        )
        .bind(user_id)
        .fetch_all(db)
        .await
        .unwrap_or_default();
        for (conn_id,) in rows {
            conns.insert(conn_id);
        }
    }
    let mut write_lock = authorized_conns.write().await;
    *write_lock = conns;
}

async fn handle_socket(
    socket: WebSocket,
    state: AppState,
    user: AuthenticatedUser,
    last_epoch: Option<String>,
    last_seq: Option<u64>,
    shutdown_token: tokio_util::sync::CancellationToken,
) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.event_journal.subscribe();

    let is_admin = user.is_admin;
    let user_id = user.id.clone();
    let db = state.db.clone();
    tracing::info!("ws.connected: user_id={}", user_id);

    // Pre-load in-memory authorized connection snapshot
    let authorized_conns = Arc::new(RwLock::new(HashSet::new()));
    reload_permissions(&db, &user_id, is_admin, &authorized_conns).await;

    // 1. Send current epoch announcement on connect
    let epoch_info = serde_json::json!({
        "type": "epoch_info",
        "data": {
            "epoch": state.event_journal.epoch(),
            "latest_sequence": state.event_journal.latest_sequence(),
        }
    });
    if sender
        .send(Message::Text(epoch_info.to_string().into()))
        .await
        .is_err()
    {
        return;
    }

    // 2. Initial replay or resync negotiation
    if let Some(seq) = last_seq {
        if let Ok(outcome) = state
            .event_journal
            .get_since(last_epoch.as_deref(), seq, 100)
            .await
        {
            match outcome {
                ReplayOutcome::Events(missed) => {
                    let conns_snapshot = authorized_conns.read().await.clone();
                    for envelope in missed {
                        if is_event_authorized(&envelope.event, &user_id, is_admin, &conns_snapshot)
                        {
                            if let Ok(json_str) = serde_json::to_string(&envelope) {
                                if sender.send(Message::Text(json_str.into())).await.is_err() {
                                    return;
                                }
                            }
                        }
                    }
                }
                ReplayOutcome::Expired { latest_sequence } => {
                    let resync = serde_json::json!({
                        "type": "resync_required",
                        "data": {
                            "reason": "sequence_expired",
                            "latest_sequence": latest_sequence,
                        }
                    });
                    let _ = sender.send(Message::Text(resync.to_string().into())).await;
                }
                ReplayOutcome::EpochMismatch {
                    current_epoch,
                    latest_sequence,
                } => {
                    let full_sync = serde_json::json!({
                        "type": "full_sync",
                        "data": {
                            "reason": "epoch_changed",
                            "epoch": current_epoch,
                            "latest_sequence": latest_sequence,
                        }
                    });
                    let _ = sender
                        .send(Message::Text(full_sync.to_string().into()))
                        .await;
                }
            }
        }
    }

    // 3. Sender task with 25s application-level heartbeat
    let shutdown_token_send = shutdown_token.clone();
    let user_id_send = user_id.clone();
    let auth_conns_send = Arc::clone(&authorized_conns);
    let db_send = db.clone();

    let mut send_task = tokio::spawn(async move {
        let mut ping_interval = tokio::time::interval(Duration::from_secs(25));
        ping_interval.tick().await;

        loop {
            tokio::select! {
                _ = shutdown_token_send.cancelled() => {
                    tracing::info!("ws.shutdown: sending close frame 1001 to user={}", user_id_send);
                    let _ = sender.send(Message::Close(Some(axum::extract::ws::CloseFrame {
                        code: axum::extract::ws::close_code::AWAY,
                        reason: "server shutting down".into(),
                    }))).await;
                    break;
                }
                _ = ping_interval.tick() => {
                    if sender.send(Message::Ping(vec![].into())).await.is_err() {
                        break;
                    }
                }
                envelope_res = rx.recv() => {
                    match envelope_res {
                        Ok(envelope) => {
                            if let crate::events::DomainEvent::PermissionChanged { user_id: ref target_user_id, .. } = envelope.event {
                                if target_user_id == &user_id_send {
                                    reload_permissions(&db_send, &user_id_send, is_admin, &auth_conns_send).await;
                                }
                            }

                            let conns = auth_conns_send.read().await;
                            if is_event_authorized(&envelope.event, &user_id_send, is_admin, &conns) {
                                if let Ok(json_str) = serde_json::to_string(&envelope) {
                                    if sender.send(Message::Text(json_str.into())).await.is_err() {
                                        break;
                                    }
                                }
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                            tracing::warn!("ws.lagged: user={} skipped={}", user_id_send, skipped);
                            let resync = serde_json::json!({
                                "type": "resync_required",
                                "data": {
                                    "reason": "buffer_overflow",
                                }
                            });
                            let _ = sender.send(Message::Text(resync.to_string().into())).await;
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            break;
                        }
                    }
                }
            }
        }
        let _ = sender.send(Message::Close(None)).await;
    });

    // 4. Receiver task handling client pongs and close frames
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Close(_) => break,
                Message::Ping(_) => {}
                Message::Pong(_) => {}
                _ => {}
            }
        }
    });

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }

    tracing::info!("ws.closed: user_id={}", user_id);
}

#[cfg(test)]
mod tests {
    use super::is_event_authorized;
    use crate::events::DomainEvent;
    use std::collections::HashSet;

    #[test]
    fn upload_progress_is_authorized_by_its_destination_only() {
        let allowed = HashSet::from(["destination".to_string()]);
        let upload = DomainEvent::TransferProgress(serde_json::json!({
            "transfer_type": "upload",
            "source_connection_id": "upload",
            "destination_connection_id": "destination"
        }));
        assert!(is_event_authorized(&upload, "user1", false, &allowed));
    }

    #[test]
    fn transfer_owner_is_always_authorized() {
        let allowed = HashSet::new();
        let owned = DomainEvent::TransferProgress(serde_json::json!({
            "user_id": "user1",
            "transfer_type": "copy",
            "source_connection_id": "source",
            "destination_connection_id": "destination"
        }));
        assert!(is_event_authorized(&owned, "user1", false, &allowed));
        assert!(!is_event_authorized(&owned, "user2", false, &allowed));
    }

    #[test]
    fn non_upload_transfer_still_requires_both_endpoints() {
        let allowed = HashSet::from(["destination".to_string()]);
        let copy = DomainEvent::TransferProgress(serde_json::json!({
            "transfer_type": "copy",
            "source_connection_id": "source",
            "destination_connection_id": "destination"
        }));
        assert!(!is_event_authorized(&copy, "other_user", false, &allowed));
    }
}
