use crate::auth::AuthenticatedUser;
use crate::db::DbPool;
use crate::state::AppState;
use crate::transfer::WsEvent;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::IntoResponse,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::Deserialize;

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
    ws.on_upgrade(move |socket| handle_socket(socket, state, user, query.last_seq))
}

async fn is_event_authorized(event: &WsEvent, is_admin: bool, user_id: &str, db: &DbPool) -> bool {
    if is_admin {
        return true;
    }

    let event_conn_ids: Vec<&str> = match event {
        WsEvent::TransferProgress(job)
        | WsEvent::TransferCompleted(job)
        | WsEvent::TransferFailed(job) => {
            vec![
                job.source_connection_id.as_str(),
                job.destination_connection_id.as_str(),
            ]
        }
        WsEvent::FileChange { connection_id, .. } => vec![connection_id.as_str()],
    };

    for conn_id in event_conn_ids {
        if conn_id == "local" {
            return true;
        }
        let has_perm: Option<(i64,)> = sqlx::query_as(
            "SELECT can_read FROM permissions WHERE user_id = ? AND connection_id = ?",
        )
        .bind(user_id)
        .bind(conn_id)
        .fetch_optional(db)
        .await
        .unwrap_or(None);

        if has_perm.map(|p| p.0 != 0).unwrap_or(false) {
            return true;
        }
    }

    false
}

async fn handle_socket(
    socket: WebSocket,
    state: AppState,
    user: AuthenticatedUser,
    last_seq: Option<u64>,
) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.transfer_manager.subscribe();

    let is_admin = user.is_admin;
    let user_id = user.id.clone();
    let db = state.db.clone();

    // 1. Initial replay of missed events if last_seq is specified (P2 #28)
    if let Some(seq) = last_seq {
        let missed = state.transfer_manager.get_events_since(seq).await;
        for envelope in missed {
            if is_event_authorized(&envelope.event, is_admin, &user_id, &db).await {
                if let Ok(json_str) = serde_json::to_string(&envelope) {
                    if sender.send(Message::Text(json_str.into())).await.is_err() {
                        return;
                    }
                }
            }
        }
    }

    // 2. Spawn live sender task
    let user_id_clone = user_id.clone();
    let db_clone = db.clone();
    let mut send_task = tokio::spawn(async move {
        while let Ok(envelope) = rx.recv().await {
            if is_event_authorized(&envelope.event, is_admin, &user_id_clone, &db_clone).await {
                if let Ok(json_str) = serde_json::to_string(&envelope) {
                    if sender.send(Message::Text(json_str.into())).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    // 3. Receive task to keep connection alive or handle client pings
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Close(_) = msg {
                break;
            }
        }
    });

    // If either task finishes, abort the other
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }
}
