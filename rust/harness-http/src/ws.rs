use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures::stream::StreamExt;
use tokio::sync::broadcast;

use harness_core::events::Event;
use harness_core::storage::EventStore;

use crate::state::AppState;

/// WebSocket upgrade handler — streams governance events in real time.
pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let mut rx = state.event_tx.subscribe();

    // Send initial snapshot of recent events
    let snapshot_msg = {
        let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
        let filter = harness_core::events::EventFilter {
            limit: Some(20),
            ..Default::default()
        };
        store.list(&filter).ok().map(|recent| {
            serde_json::json!({
                "type": "snapshot",
                "events": recent,
            })
        })
    };
    if let Some(snapshot) = snapshot_msg {
        let _ = socket
            .send(Message::Text(
                serde_json::to_string(&snapshot).unwrap_or_default().into(),
            ))
            .await;
    }

    // Stream new events as they arrive
    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(event) => {
                        let msg = serde_json::json!({
                            "type": "event",
                            "event": event,
                        });
                        if socket.send(Message::Text(
                            serde_json::to_string(&msg).unwrap_or_default().into()
                        )).await.is_err() {
                            break; // client disconnected
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        let msg = serde_json::json!({
                            "type": "lagged",
                            "missed": n,
                        });
                        let _ = socket.send(Message::Text(
                            serde_json::to_string(&msg).unwrap_or_default().into()
                        )).await;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            msg = socket.next() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(data))) => {
                        let _ = socket.send(Message::Pong(data)).await;
                    }
                    _ => {} // ignore other messages
                }
            }
        }
    }
}

/// Broadcast an event to all connected WebSocket clients.
pub fn broadcast_event(tx: &broadcast::Sender<Event>, event: &Event) {
    // Ignore send errors (no subscribers connected)
    let _ = tx.send(event.clone());
}
