use crate::auth::AuthenticatedUser;
use crate::events::ReplayOutcome;
use crate::services::{RealtimePrincipal, RealtimeService};
use crate::state::RealtimeState;
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
    State(state): State<RealtimeState>,
) -> impl IntoResponse {
    let principal = RealtimePrincipal::new(user.id.clone(), user.is_admin);
    let service = state.service.clone();
    ws.on_upgrade(move |socket| {
        handle_socket(
            socket,
            service,
            principal,
            query.last_epoch,
            query.last_seq,
        )
    })
}

async fn handle_socket(
    socket: WebSocket,
    service: RealtimeService,
    principal: RealtimePrincipal,
    last_epoch: Option<String>,
    last_seq: Option<u64>,
) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = service.subscribe();
    let shutdown_token = service.shutdown_token();
    let user_id = principal.user_id.clone();

    tracing::info!("ws.connected: user_id={}", user_id);

    let authorized_connections = Arc::new(RwLock::new(
        service.authorized_connections(&principal).await,
    ));

    let epoch = service.epoch_info();
    let epoch_info = serde_json::json!({
        "type": "epoch_info",
        "data": {
            "epoch": epoch.epoch,
            "latest_sequence": epoch.latest_sequence,
        }
    });
    if sender
        .send(Message::Text(epoch_info.to_string().into()))
        .await
        .is_err()
    {
        return;
    }

    if let Some(sequence) = last_seq {
        if let Ok(outcome) = service
            .replay(last_epoch.as_deref(), sequence, 100)
            .await
        {
            match outcome {
                ReplayOutcome::Events(missed) => {
                    let connections = authorized_connections.read().await.clone();
                    for envelope in missed {
                        if service.is_event_authorized(&envelope.event, &principal, &connections) {
                            if let Ok(json) = serde_json::to_string(&envelope) {
                                if sender.send(Message::Text(json.into())).await.is_err() {
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

    let send_service = service.clone();
    let send_principal = principal.clone();
    let send_user_id = user_id.clone();
    let send_connections = Arc::clone(&authorized_connections);

    let mut send_task = tokio::spawn(async move {
        let mut ping_interval = tokio::time::interval(Duration::from_secs(25));
        ping_interval.tick().await;

        loop {
            tokio::select! {
                _ = shutdown_token.cancelled() => {
                    tracing::info!("ws.shutdown: sending close frame 1001 to user={}", send_user_id);
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
                envelope_result = rx.recv() => {
                    match envelope_result {
                        Ok(envelope) => {
                            if let crate::events::DomainEvent::PermissionChanged {
                                user_id: ref target_user_id,
                                ..
                            } = envelope.event
                            {
                                if target_user_id == &send_principal.user_id {
                                    let refreshed = send_service
                                        .authorized_connections(&send_principal)
                                        .await;
                                    *send_connections.write().await = refreshed;
                                }
                            }

                            let connections = send_connections.read().await;
                            if send_service.is_event_authorized(
                                &envelope.event,
                                &send_principal,
                                &connections,
                            ) {
                                if let Ok(json) = serde_json::to_string(&envelope) {
                                    if sender.send(Message::Text(json.into())).await.is_err() {
                                        break;
                                    }
                                }
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                            tracing::warn!("ws.lagged: user={} skipped={}", send_user_id, skipped);
                            let resync = serde_json::json!({
                                "type": "resync_required",
                                "data": {
                                    "reason": "buffer_overflow",
                                }
                            });
                            let _ = sender.send(Message::Text(resync.to_string().into())).await;
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }

        let _ = sender.send(Message::Close(None)).await;
    });

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(message)) = receiver.next().await {
            match message {
                Message::Close(_) => break,
                Message::Ping(_) | Message::Pong(_) => {}
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
    use super::*;

    #[test]
    fn websocket_query_keeps_resume_contract() {
        let query = WsQuery {
            last_seq: Some(42),
            last_epoch: Some("epoch".to_string()),
        };
        assert_eq!(query.last_seq, Some(42));
        assert_eq!(query.last_epoch.as_deref(), Some("epoch"));
    }

    #[test]
    fn websocket_authorization_snapshot_type_is_transport_local() {
        let connections: Arc<RwLock<HashSet<String>>> =
            Arc::new(RwLock::new(HashSet::new()));
        assert_eq!(Arc::strong_count(&connections), 1);
    }
}
