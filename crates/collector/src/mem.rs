//! Memory metrics collector.
//!
//! Data sources:
//!   `/proc/meminfo`                    — RAM stats
//!   `/proc/spl/kstat/zfs/arcstats`    — ZFS ARC cache (optional)
//!   `dmidecode -t memory`              — physical DIMM info (requires root)

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemStats {
    pub total: u64,
    pub free: u64,
    pub available: u64,
    pub buffers: u64,
    pub cached: u64,
    pub used: u64,
    pub swap_total: u64,
    pub swap_free: u64,
    pub swap_used: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zfs_arc: Option<u64>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub physical_ram: Vec<String>,
}

/// Collects memory statistics, replicating btop's `Mem::collect()`.
pub fn collect() -> Result<MemStats> {
    let mut stats = MemStats::default();
    collect_meminfo(&mut stats);
    collect_zfs_arc(&mut stats);
    stats.physical_ram = collect_dimm_info();
    Ok(stats)
}

fn collect_meminfo(stats: &mut MemStats) {
    let Ok(content) = std::fs::read_to_string("/proc/meminfo") else { return };
    let mut free_raw: u64 = 0;
    let mut got_avail = false;

    for line in content.lines() {
        let mut parts = line.split_whitespace();
        let key = parts.next().unwrap_or("");
        let val: u64 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
        let bytes = val << 10; // kB → bytes

        match key {
            "MemTotal:"     => stats.total = bytes,
            "MemFree:"      => { free_raw = bytes; stats.free = bytes; }
            "MemAvailable:" => { stats.available = bytes; got_avail = true; }
            "Cached:"       => stats.cached = bytes,
            "Buffers:"      => stats.buffers = bytes,
            "SwapTotal:"    => stats.swap_total = bytes,
            "SwapFree:"     => stats.swap_free = bytes,
            _ => {}
        }
    }

    // btop: if MemAvailable absent, approximate as Free + Cached
    if !got_avail {
        stats.available = free_raw + stats.cached;
    }
    // btop: Used = Total - (Available <= Total ? Available : Free)
    stats.used = if stats.available <= stats.total {
        stats.total - stats.available
    } else {
        stats.total - free_raw
    };
    stats.swap_used = stats.swap_total.saturating_sub(stats.swap_free);
}

/// Reads ZFS ARC stats and adjusts `cached` / `available` accordingly (btop logic).
fn collect_zfs_arc(stats: &mut MemStats) {
    let Ok(content) = std::fs::read_to_string("/proc/spl/kstat/zfs/arcstats") else { return };
    let mut arc_size: u64 = 0;
    let mut arc_min: u64 = 0;

    for line in content.lines() {
        let mut parts = line.split_whitespace();
        let name = parts.next().unwrap_or("");
        let val: u64 = parts.nth(1).and_then(|v| v.parse().ok()).unwrap_or(0);
        match name {
            "size"  => arc_size = val,
            "c_min" => arc_min  = val,
            _ => {}
        }
    }

    if arc_size > 0 {
        stats.zfs_arc = Some(arc_size);
        stats.cached += arc_size;
        // ARC won't shrink below c_min, so only the shrinkable portion counts
        if arc_size > arc_min {
            stats.available += arc_size - arc_min;
        }
    }
}

/// Runs `dmidecode -t memory` and parses populated DIMM entries.
/// Silently returns an empty vec if dmidecode is missing or permission denied.
fn collect_dimm_info() -> Vec<String> {
    let Ok(output) = std::process::Command::new("dmidecode")
        .args(["-t", "memory"])
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() { return Vec::new(); }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut dimms: Vec<String> = Vec::new();
    let (mut size, mut kind, mut speed, mut mfr) =
        (String::new(), String::new(), String::new(), String::new());

    for line in text.lines() {
        let line = line.trim();
        if line == "Memory Device" {
            commit_dimm(&mut dimms, &size, &kind, &speed, &mfr);
            size.clear(); kind.clear(); speed.clear(); mfr.clear();
        } else if let Some(v) = line.strip_prefix("Size:") {
            size = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("Type:") {
            if !line.starts_with("Type Detail:") {
                kind = v.trim().to_string();
            }
        } else if let Some(v) = line.strip_prefix("Speed:") {
            speed = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("Manufacturer:") {
            mfr = v.trim().to_string();
        }
    }
    commit_dimm(&mut dimms, &size, &kind, &speed, &mfr);
    dimms
}

fn commit_dimm(dimms: &mut Vec<String>, size: &str, kind: &str, speed: &str, mfr: &str) {
    if size.is_empty() || size.contains("No Module Installed") { return; }
    let entry = format!("{size} {kind} {speed} {mfr}")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if !entry.contains("Unknown") {
        dimms.push(entry);
    }
}
