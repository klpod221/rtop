//! Agent daemon — collects metrics on a ticker and POSTs them to a remote endpoint.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tokio::signal::unix::{signal, SignalKind};
use tokio::time::{interval, Duration};
use tracing::{error, info, warn};

use app_collector::gpu::intel::IntelGpuCollector;
use app_config;

use super::sender::Sender;

/// Controls a single agent run.
pub struct RunOptions {
    /// Override the default config path.
    pub config_path: Option<PathBuf>,
    /// Print payload to stderr instead of POSTing.
    pub dry_run: bool,
    /// Exit after a single collection cycle.
    pub once: bool,
}

/// Entry point for `rtop agent`. Runs until SIGTERM/SIGINT.
pub async fn run(opts: RunOptions) -> Result<()> {
    let cfg_path = resolve_config_path(opts.config_path.as_deref())?;
    let cfg = app_config::load(&cfg_path).context("loading config")?;

    let machine_id = resolve_machine_id(&cfg.agent.machine_id);
    let machine_name = resolve_machine_name(&cfg.agent.machine_name);

    write_pid(&cfg.agent.pid_file)?;
    let _pid_guard = PidGuard(cfg.agent.pid_file.clone());

    let sender = if opts.dry_run {
        None
    } else {
        Some(Sender::new(&cfg.server).context("creating HTTP sender")?)
    };

    // Warm-up: initialise stateful collectors
    #[cfg(feature = "nvidia")]
    app_collector::gpu::nvidia::init();

    let mut intel_col: Option<IntelGpuCollector> = None;
    if cfg.modules.gpu.enabled && cfg.modules.gpu.intel {
        match IntelGpuCollector::new() {
            Ok(col) => {
                intel_col = Some(col);
                // Warm-up tick so the next call gives valid deltas
                if let Some(ref mut c) = intel_col {
                    c.collect();
                }
            }
            Err(e) => {
                let msg = e.to_string();
                if !msg.contains("no Intel GPU PMU device found")
                    && !msg.contains("no GPU engines discovered")
                {
                    warn!("Intel GPU init: {e}. Hint: sudo setcap cap_perfmon,cap_dac_read_search=ep <binary>");
                }
            }
        }
    }

    // CPU needs one baseline tick before deltas are valid
    if cfg.modules.cpu.enabled {
        let _ = app_collector::cpu::collect();
    }

    let mut ticker = interval(Duration::from_secs(cfg.agent.interval_seconds));
    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;

    info!(
        "rtop-agent started (interval={}s, dry_run={})",
        cfg.agent.interval_seconds, opts.dry_run
    );

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let payload = app_collector::collect_all(
                    &cfg,
                    intel_col.as_mut(),
                    machine_id.as_deref(),
                    machine_name.as_deref(),
                );
                let json = serde_json::to_vec(&payload).context("serialising payload")?;

                if opts.dry_run {
                    eprintln!("[dry-run] {} bytes\n{}", json.len(), serde_json::to_string_pretty(&payload).unwrap_or_default());
                } else if let Some(ref s) = sender {
                    if let Err(e) = s.send(&json).await {
                        error!("send failed: {e}");
                    } else {
                        info!("sent {} bytes to {}", json.len(), cfg.server.endpoint);
                    }
                }

                if opts.once { return Ok(()); }
            }
            _ = sigterm.recv() => { info!("SIGTERM received — shutting down"); return Ok(()); }
            _ = sigint.recv()  => { info!("SIGINT received — shutting down");  return Ok(()); }
        }
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn resolve_config_path(path: Option<&Path>) -> Result<PathBuf> {
    if let Some(p) = path {
        return Ok(p.to_path_buf());
    }
    app_config::default_config_path().map_err(|e| anyhow::anyhow!("{e}"))
}

fn resolve_machine_id(configured: &str) -> Option<String> {
    if !configured.is_empty() {
        return Some(configured.to_string());
    }
    std::fs::read_to_string("/etc/machine-id")
        .ok()
        .map(|s| s.trim().to_string())
}

fn resolve_machine_name(configured: &str) -> Option<String> {
    if !configured.is_empty() {
        return Some(configured.to_string());
    }
    hostname::get().ok().and_then(|h| h.into_string().ok())
}

fn write_pid(path: &str) -> Result<()> {
    if path.is_empty() {
        return Ok(());
    }
    std::fs::write(path, format!("{}\n", std::process::id()))
        .with_context(|| format!("writing PID to {path}"))?;
    Ok(())
}

/// RAII guard that removes the PID file on drop.
struct PidGuard(String);
impl Drop for PidGuard {
    fn drop(&mut self) {
        if !self.0.is_empty() {
            let _ = std::fs::remove_file(&self.0);
        }
    }
}
