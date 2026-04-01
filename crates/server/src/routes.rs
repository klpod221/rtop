//! HTTP route definitions for the web server.

use std::sync::{Arc, Mutex};

use axum::{
    extract::{ws::WebSocketUpgrade, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use tower_http::cors::CorsLayer;

use app_collector::{collect_all, gpu::intel::IntelGpuCollector};
use app_config::{self as cfg_lib, Config};

use super::assets::WebAssets;
use super::ws::{spawn_broadcast_loop, ws_handler, MetricsBroadcast};

// ─── App state ─────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    broadcast: MetricsBroadcast,
    cfg: Arc<Config>,
}

// ─── Router builder ────────────────────────────────────────────────────────────

pub async fn build_router(
    cfg: Config,
    intel_col: Option<IntelGpuCollector>,
) -> Router {
    let cfg = Arc::new(cfg);
    let cfg_for_collect = cfg.clone();
    let intel_col = Arc::new(Mutex::new(intel_col));

    // Spawn the 1-second broadcast loop
    let broadcast = spawn_broadcast_loop(move || {
        let mut guard = intel_col.lock().unwrap();
        collect_all(&cfg_for_collect, guard.as_mut(), None, None)
    });

    let state = AppState { broadcast, cfg };

    Router::new()
        .route("/ws",         get(ws_route))
        .route("/api/config", get(get_config).post(post_config))
        .fallback(static_handler)          // SPA fallback
        .layer(CorsLayer::permissive())
        .with_state(state)
}

// ─── WebSocket route ──────────────────────────────────────────────────────────

async fn ws_route(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| ws_handler(socket, state.broadcast))
}

// ─── Config API ───────────────────────────────────────────────────────────────

async fn get_config(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.cfg.web.clone())
}

async fn post_config(
    State(_state): State<AppState>,
    Json(web_patch): Json<app_config::WebConfig>,
) -> impl IntoResponse {
    let path = match cfg_lib::default_config_path() {
        Ok(p) => p,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };
    let mut full = match cfg_lib::load(&path) {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };
    if web_patch.port != 0 {
        full.web = web_patch;
    } else {
        let preserved_port = full.web.port;
        full.web = web_patch;
        full.web.port = preserved_port;
    }
    if let Err(e) = cfg_lib::write(&path, &full) {
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
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
            ).into_response()
        }
        None => {
            // SPA fallback — serve index.html for any unknown route
            match WebAssets::get("index.html") {
                Some(index) => (
                    [(axum::http::header::CONTENT_TYPE, "text/html")],
                    index.data,
                ).into_response(),
                None => StatusCode::NOT_FOUND.into_response(),
            }
        }
    }
}
