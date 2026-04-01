//! WebSocket broadcast loop — pushes a telemetry snapshot to all connected clients
//! at the interval configured in `Config::update_interval_ms`.

use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, warn};
use serde::Deserialize;

use app_collector::AgentPayload;
use app_config::Config;

pub type MetricsBroadcast = broadcast::Sender<Arc<AgentPayload>>;

/// Spawns the background ticker that collects metrics and broadcasts them.
///
/// The tick interval is read from `cfg.update_interval_ms` on each iteration,
/// so changes propagate immediately without restarting.
pub fn spawn_broadcast_loop<F>(cfg: Arc<RwLock<Config>>, collect: F) -> MetricsBroadcast
where
    F: Fn(&Config) -> AgentPayload + Send + 'static,
{
    let (tx, _) = broadcast::channel::<Arc<AgentPayload>>(4);
    let tx_clone = tx.clone();

    tokio::spawn(async move {
        loop {
            let ms = {
                let c = cfg.read().await;
                c.update_interval_ms.max(100)
            };
            tokio::time::sleep(Duration::from_millis(ms)).await;

            let payload = {
                let c = cfg.read().await;
                collect(&c)
            };
            // Ignore send errors — no subscribers is fine
            let _ = tx_clone.send(Arc::new(payload));
        }
    });

    tx
}

// ─── Control Message ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct ProcessFilter {
    #[serde(default)]
    pub search: String,
    #[serde(default = "default_sort_by")]
    pub sort_by: String,
    #[serde(default = "default_sort_desc")]
    pub sort_desc: bool,
    #[serde(default)]
    pub user: String,
}

fn default_sort_by() -> String { "cpu".to_string() }
const fn default_sort_desc() -> bool { true }

impl Default for ProcessFilter {
    fn default() -> Self {
        Self {
            search: String::new(),
            sort_by: default_sort_by(),
            sort_desc: default_sort_desc(),
            user: String::new(),
        }
    }
}

// ─── Handler ──────────────────────────────────────────────────────────────────

/// Handles a single WebSocket connection — subscribes to the broadcast and
/// forwards serialized JSON until the client disconnects.
pub async fn ws_handler(socket: WebSocket, tx: MetricsBroadcast) {
    let mut rx = tx.subscribe();
    let (mut sender, mut receiver) = socket.split();

    let filter = Arc::new(RwLock::new(ProcessFilter::default()));

    // Background task: serialize payload per-connection then forward → client
    let filter_clone = filter.clone();
    let mut send_task = tokio::spawn(async move {
        while let Ok(payload_arc) = rx.recv().await {
            // Fast path: if no subscribers, avoid clone? We are the subscriber.
            // We must clone because we mutate the payload for our specific filters.
            let mut payload = (*payload_arc).clone();

            let f = filter_clone.read().await.clone();

            // 1. Filter
            if !f.search.is_empty() {
                payload.processes = app_collector::proc::filter_by_name(&payload.processes, &f.search);
            }
            if !f.user.is_empty() {
                payload.processes.retain(|p| p.user == f.user);
            }

            // 2. Sort
            if !f.sort_by.is_empty() {
                app_collector::proc::sort(&mut payload.processes, &f.sort_by);
                if !f.sort_desc {
                    payload.processes.reverse();
                }
            }

            match serde_json::to_string(&payload) {
                Ok(json) => {
                    if sender.send(Message::Text(json.into())).await.is_err() {
                        break;
                    }
                }
                Err(e) => warn!("payload serialise error: {e}"),
            }
        }
    });

    // Read loop to detect client disconnect and control messages
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(text) = msg {
                if let Ok(new_filter) = serde_json::from_str::<ProcessFilter>(&text) {
                    *filter.write().await = new_filter;
                }
            }
        }
    });

    // Wait for either side to finish
    tokio::select! {
        _ = &mut send_task => { recv_task.abort(); }
        _ = &mut recv_task => { send_task.abort(); }
    }
    debug!("WebSocket client disconnected");
}
