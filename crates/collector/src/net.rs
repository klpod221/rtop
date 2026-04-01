//! Network interface metrics collector.
//!
//! Data source: `/proc/net/dev` for byte/packet counters (delta-based speeds).

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use anyhow::Result;
use serde::{Deserialize, Serialize};

// ─── Public types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetInterface {
    pub name: String,
    pub rx_bytes_total: u64,
    pub tx_bytes_total: u64,
    /// Receive bytes/s since last sample.
    pub rx_bytes_per_sec: f64,
    /// Transmit bytes/s since last sample.
    pub tx_bytes_per_sec: f64,
    pub rx_packets_total: u64,
    pub tx_packets_total: u64,
    /// Interface type hint (ethernet / wifi / virtual / loopback / other).
    pub iface_type: String,
}

// ─── Delta state ──────────────────────────────────────────────────────────────

struct NetSnapshot {
    rx: u64,
    tx: u64,
    when: Instant,
}

static NET_STATE: OnceLock<Mutex<HashMap<String, NetSnapshot>>> = OnceLock::new();

fn net_state() -> &'static Mutex<HashMap<String, NetSnapshot>> {
    NET_STATE.get_or_init(|| Mutex::new(HashMap::new()))
}

// ─── Collector ────────────────────────────────────────────────────────────────

/// Collects all network interface counters and returns per-interface stats.
pub fn collect(iface_filter: &[String], exclude_virtual: bool) -> Result<Vec<NetInterface>> {
    let content = std::fs::read_to_string("/proc/net/dev")?;
    let now = Instant::now();
    let mut state = net_state().lock().unwrap();
    let mut results = Vec::new();

    for line in content.lines().skip(2) {
        let line = line.trim();
        let Some(colon) = line.find(':') else { continue };
        let name = line[..colon].trim().to_string();

        let fields: Vec<u64> = line[colon + 1..]
            .split_whitespace()
            .map(|f| f.parse().unwrap_or(0))
            .collect();
        if fields.len() < 9 { continue; }

        let rx = fields[0];
        let rx_pkts = fields[1];
        let tx = fields[8];
        let tx_pkts = fields[9];

        // Apply filters
        if !iface_filter.is_empty() && !iface_filter.iter().any(|f| f == &name) { continue; }
        if exclude_virtual && is_virtual_iface(&name) { continue; }

        let (rx_bps, tx_bps) = if let Some(prev) = state.get(&name) {
            let dt = now.duration_since(prev.when).as_secs_f64();
            if dt > 0.0 {
                (
                    rx.saturating_sub(prev.rx) as f64 / dt,
                    tx.saturating_sub(prev.tx) as f64 / dt,
                )
            } else { (0.0, 0.0) }
        } else { (0.0, 0.0) };

        state.insert(name.clone(), NetSnapshot { rx, tx, when: now });

        results.push(NetInterface {
            iface_type: classify_iface(&name).to_string(),
            name,
            rx_bytes_total: rx,
            tx_bytes_total: tx,
            rx_bytes_per_sec: rx_bps,
            tx_bytes_per_sec: tx_bps,
            rx_packets_total: rx_pkts,
            tx_packets_total: tx_pkts,
        });
    }
    Ok(results)
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn is_virtual_iface(name: &str) -> bool {
    name == "lo"
        || name.starts_with("br-")
        || name.starts_with("veth")
        || name.starts_with("docker")
        || name.starts_with("virbr")
        || name.starts_with("tailscale")
        || name.starts_with("tun")
        || name.starts_with("wg")
        || name.starts_with("tap")
}

fn classify_iface(name: &str) -> &'static str {
    if name == "lo" { return "loopback"; }
    if name.starts_with("eth") || name.starts_with("en") || name.starts_with("eno") { return "ethernet"; }
    if name.starts_with("wl") || name.starts_with("wifi") { return "wifi"; }
    if is_virtual_iface(name) { return "virtual"; }
    "other"
}
