//! Web UI + WebSocket broadcast server.

pub mod assets;
pub mod routes;
pub mod ws;

use std::net::SocketAddr;

use anyhow::Result;
use tokio::net::TcpListener;
use tracing::info;

use app_collector::gpu::intel::IntelGpuCollector;
use app_config::Config;

pub use routes::build_router;

/// Starts the HTTP server on the configured port. Blocks until the server stops.
pub async fn start(cfg: &Config, cfg_path: std::path::PathBuf) -> Result<()> {
    let port = cfg.web.port;

    // Warm-up GPU collectors
    #[cfg(feature = "nvidia")]
    app_collector::gpu::nvidia::init();

    let mut intel_col: Option<IntelGpuCollector> = match IntelGpuCollector::new() {
        Ok(c) => { tracing::info!("Intel GPU PMU initialised"); Some(c) }
        Err(e) => { tracing::debug!("Intel GPU: {e}"); None }
    };
    if let Some(ref mut c) = intel_col { c.collect(); }
    app_collector::cpu::collect().ok(); // baseline tick

    let cfg_clone = cfg.clone();
    let router = build_router(cfg_clone, cfg_path, intel_col).await;

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Starting Web UI on http://localhost:{port}");
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, router).await?;
    Ok(())
}
