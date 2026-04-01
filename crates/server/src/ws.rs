//! WebSocket broadcast loop — pushes a telemetry snapshot to all connected clients
//! once per second using a `tokio::sync::broadcast` channel.

use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::broadcast;
use tokio::time::interval;
use tracing::{debug, warn};

use app_collector::AgentPayload;

pub type MetricsBroadcast = broadcast::Sender<Arc<String>>;

/// Spawns the background ticker that collects metrics and broadcasts them.
/// Returns a `broadcast::Sender` that WebSocket handlers can subscribe to.
pub fn spawn_broadcast_loop<F>(collect: F) -> MetricsBroadcast
where
    F: Fn() -> AgentPayload + Send + 'static,
{
    let (tx, _) = broadcast::channel::<Arc<String>>(4);
    let tx_clone = tx.clone();

    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(1));
        loop {
            ticker.tick().await;
            let payload = collect();
            match serde_json::to_string(&payload) {
                Ok(json) => {
                    // Ignore send errors — no subscribers is fine
                    let _ = tx_clone.send(Arc::new(json));
                }
                Err(e) => warn!("payload serialise error: {e}"),
            }
        }
    });

    tx
}

/// Handles a single WebSocket connection — subscribes to the broadcast and
/// forwards messages until the client disconnects.
pub async fn ws_handler(socket: WebSocket, tx: MetricsBroadcast) {
    let mut rx = tx.subscribe();
    let (mut sender, mut receiver) = socket.split();

    // Background task: forward broadcast → client
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg.as_str().into())).await.is_err() {
                break;
            }
        }
    });

    // Read loop to detect client disconnect
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(_)) = receiver.next().await {}
    });

    // Wait for either side to finish
    tokio::select! {
        _ = &mut send_task => { recv_task.abort(); }
        _ = &mut recv_task => { send_task.abort(); }
    }
    debug!("WebSocket client disconnected");
}
