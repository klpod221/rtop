//! HTTP route definitions for the web server.

use std::sync::{Arc, Mutex};

use axum::{
    extract::{ws::WebSocketUpgrade, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;

use app_collector::{collect_all, gpu::intel::IntelGpuCollector};
use app_config::{self as cfg_lib, Config};

use super::assets::WebAssets;
use super::ws::{spawn_broadcast_loop, ws_handler, MetricsBroadcast};

// ─── App state ─────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    broadcast: MetricsBroadcast,
    cfg: Arc<RwLock<Config>>,
}

// ─── Config API DTO ────────────────────────────────────────────────────────────

/// Flattened view of settings exposed over the API.
/// Combines `WebConfig` fields with top-level `update_interval_ms`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConfigDto {
    pub port: u16,
    pub network_interface: String,
    pub storage_filter: Vec<String>,
    /// Shared refresh interval (ms, min 100).
    pub update_interval_ms: u64,
}

impl From<&Config> for ConfigDto {
    fn from(c: &Config) -> Self {
        Self {
            port: c.web.port,
            network_interface: c.web.network_interface.clone(),
            storage_filter: c.web.storage_filter.clone(),
            update_interval_ms: c.update_interval_ms,
        }
    }
}

// ─── Router builder ────────────────────────────────────────────────────────────

pub async fn build_router(
    cfg: Config,
    intel_col: Option<IntelGpuCollector>,
) -> Router {
    let cfg = Arc::new(RwLock::new(cfg));
    let intel_col = Arc::new(Mutex::new(intel_col));

    let broadcast = spawn_broadcast_loop(cfg.clone(), move |c| {
        let mut guard = intel_col.lock().unwrap();
        collect_all(c, guard.as_mut(), None, None)
    });

    let state = AppState { broadcast, cfg };

    Router::new()
        .route("/ws",         get(ws_route))
        .route("/api/config", get(get_config).post(post_config))
        .fallback(static_handler)
        .layer(CorsLayer::permissive())
        .with_state(state)
}

// ─── WebSocket route ──────────────────────────────────────────────────────────

async fn ws_route(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| ws_handler(socket, state.broadcast))
}

// ─── Config API ───────────────────────────────────────────────────────────────

async fn get_config(State(state): State<AppState>) -> impl IntoResponse {
    let cfg = state.cfg.read().await;
    Json(ConfigDto::from(&*cfg))
}

async fn post_config(
    State(state): State<AppState>,
    Json(dto): Json<ConfigDto>,
) -> impl IntoResponse {
    if dto.update_interval_ms < 100 {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            "update_interval_ms must be >= 100",
        )
            .into_response();
    }

    // Persist to disk
    let path = match cfg_lib::default_config_path() {
        Ok(p) => p,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };
    let mut full = match cfg_lib::load(&path) {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    let preserved_port = if dto.port == 0 { full.web.port } else { dto.port };
    full.web.port = preserved_port;
    full.web.network_interface = dto.network_interface.clone();
    full.web.storage_filter = dto.storage_filter.clone();
    full.update_interval_ms = dto.update_interval_ms;

    if let Err(e) = cfg_lib::write(&path, &full) {
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    }

    // Update in-memory immediately so GET reflects the change right away
    {
        let mut cfg = state.cfg.write().await;
        cfg.web.port = full.web.port;
        cfg.web.network_interface = full.web.network_interface;
        cfg.web.storage_filter = full.web.storage_filter;
        cfg.update_interval_ms = full.update_interval_ms;
    }

    Json(serde_json::json!({"status": "ok"})).into_response()
}

// ─── Static file handler (SPA) ────────────────────────────────────────────────

async fn static_handler(uri: axum::http::Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    match WebAssets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                [(axum::http::header::CONTENT_TYPE, mime.as_ref())],
                content.data,
            )
                .into_response()
        }
        None => match WebAssets::get("index.html") {
            Some(index) => (
                [(axum::http::header::CONTENT_TYPE, "text/html")],
                index.data,
            )
                .into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        },
    }
}
