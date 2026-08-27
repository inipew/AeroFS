use crate::auth::AuthenticatedUser;
use crate::state::AppState;
use crate::transfer::WsEvent;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use futures::{sink::SinkExt, stream::StreamExt};

pub async fn ws_handler(
    user: AuthenticatedUser,
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state, user))
}

async fn handle_socket(socket: WebSocket, state: AppState, user: AuthenticatedUser) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.transfer_manager.subscribe();

    let is_admin = user.is_admin;
    let user_id = user.id.clone();
    let db = state.db.clone();

    // Spawn a task that sends broadcast events to the websocket client with permission scoping
    let mut send_task = tokio::spawn(async move {
        while let Ok(envelope) = rx.recv().await {
            // Check event connection scope
            let event_conn_id: Option<&str> = match &envelope.event {
                WsEvent::TransferProgress(job) => Some(&job.source_connection_id),
                WsEvent::TransferCompleted(job) => Some(&job.source_connection_id),
                WsEvent::TransferFailed(job) => Some(&job.source_connection_id),
                WsEvent::FileChange { connection_id, .. } => Some(connection_id.as_str()),
            };

            let is_authorized = if is_admin {
                true
            } else if let Some(conn_id) = event_conn_id {
                if conn_id == "local" {
                    true
                } else {
                    let has_perm: Option<(i64,)> = sqlx::query_as(
                        "SELECT can_read FROM permissions WHERE user_id = ? AND connection_id = ?",
                    )
                    .bind(&user_id)
                    .bind(conn_id)
                    .fetch_optional(&db)
                    .await
                    .unwrap_or(None);

                    has_perm.map(|p| p.0 != 0).unwrap_or(false)
                }
            } else {
                true
            };

            if is_authorized {
                if let Ok(json_str) = serde_json::to_string(&envelope) {
                    if sender.send(Message::Text(json_str.into())).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    // Receive task to keep connection alive or handle client pings
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
