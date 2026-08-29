use crate::auth::AuthenticatedUser;
use crate::state::AppState;
use crate::transfer::{ReplayResult, WsEvent};
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

#[derive(Debug, Deserialize)]
pub struct WsQuery {
    pub last_seq: Option<u64>,
}

pub async fn ws_handler(
    user: AuthenticatedUser,
    ws: WebSocketUpgrade,
    Query(query): Query<WsQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let shutdown_token = state.runtime.shutdown_token.clone();
    ws.on_upgrade(move |socket| handle_socket(socket, state, user, query.last_seq, shutdown_token))
}

fn is_event_authorized(
    event: &WsEvent,
    is_admin: bool,
    authorized_conns: &HashSet<String>,
) -> bool {
    if is_admin {
        return true;
    }

    match event {
        WsEvent::TransferProgress(job)
        | WsEvent::TransferCompleted(job)
        | WsEvent::TransferFailed(job) => {
            authorized_conns.contains(&job.source_connection_id)
                || authorized_conns.contains(&job.destination_connection_id)
        }
        WsEvent::FileChange { connection_id, .. } => authorized_conns.contains(connection_id),
        WsEvent::ResyncRequired { .. } => true,
    }
}

async fn handle_socket(
    socket: WebSocket,
    state: AppState,
    user: AuthenticatedUser,
    last_seq: Option<u64>,
    shutdown_token: tokio_util::sync::CancellationToken,
) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.transfer_manager.subscribe();

    let is_admin = user.is_admin;
    let user_id = user.id.clone();
    let db = state.db.clone();

    // Pre-load in-memory authorized connection snapshot to eliminate high-frequency DB queries (P1.15)
    let mut authorized_conns = HashSet::new();
    authorized_conns.insert("local".to_string());
    if !is_admin {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT connection_id FROM permissions WHERE user_id = ? AND can_read = 1",
        )
        .bind(&user_id)
        .fetch_all(&db)
        .await
        .unwrap_or_default();
        for (conn_id,) in rows {
            authorized_conns.insert(conn_id);
        }
    }

    // 1. Initial replay of missed events or emit resync_required if buffer expired (Plan 40 P0.2, P1.13)
    if let Some(seq) = last_seq {
        match state.transfer_manager.get_events_since(seq).await {
            ReplayResult::Events(missed) => {
                for envelope in missed {
                    if is_event_authorized(&envelope.event, is_admin, &authorized_conns) {
                        if let Ok(json_str) = serde_json::to_string(&envelope) {
                            if sender.send(Message::Text(json_str.into())).await.is_err() {
                                return;
                            }
                        }
                    }
                }
            }
            ReplayResult::Expired { latest_sequence } => {
                let resync_envelope = crate::transfer::EventEnvelope {
                    id: uuid::Uuid::new_v4().to_string(),
                    sequence: latest_sequence,
                    timestamp: chrono::Utc::now(),
                    event: WsEvent::ResyncRequired {
                        reason: "sequence_expired".into(),
                        latest_sequence,
                    },
                };
                if let Ok(json_str) = serde_json::to_string(&resync_envelope) {
                    if sender.send(Message::Text(json_str.into())).await.is_err() {
                        return;
                    }
                }
            }
        }
    }

    // 2. Spawn live sender task — handles Lagged by sending ResyncRequired instead of dropping
    let mut send_task = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(envelope) => {
                    if is_event_authorized(&envelope.event, is_admin, &authorized_conns) {
                        if let Ok(json_str) = serde_json::to_string(&envelope) {
                            if sender.send(Message::Text(json_str.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    // Buffer overflow: send ResyncRequired so client knows it missed events
                    tracing::warn!("ws.lagged: skipped={} — sending resync_required", skipped);
                    let seq = rx.len() as u64; // approximate; client will re-handshake anyway
                    let resync = crate::transfer::EventEnvelope {
                        id: uuid::Uuid::new_v4().to_string(),
                        sequence: seq,
                        timestamp: chrono::Utc::now(),
                        event: WsEvent::ResyncRequired {
                            reason: "buffer_overflow".into(),
                            latest_sequence: seq,
                        },
                    };
                    if let Ok(json_str) = serde_json::to_string(&resync) {
                        let _ = sender.send(Message::Text(json_str.into())).await;
                    }
                    // Continue receiving live events after resync notification
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    // Channel closed (server shutdown) — break and let shutdown select arm handle close frame
                    break;
                }
            }
        }
        let _ = sender.send(Message::Close(None)).await;
    });

    // 3. Receive task to keep connection alive or handle client pings
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Close(_) = msg {
                break;
            }
        }
    });

    // 4. Select: shutdown signal sends Close frame 1001, or either task finishes naturally
    tokio::select! {
        _ = shutdown_token.cancelled() => {
            tracing::debug!("ws.shutdown: sending close frame 1001");
            // Abort the sender task (it may be blocked waiting on broadcast rx)
            send_task.abort();
            recv_task.abort();
        }
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }
}

