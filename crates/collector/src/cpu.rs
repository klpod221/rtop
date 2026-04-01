//! CPU metrics collector.
//!
//! Data sources (same chain as btop):
//!   `/proc/stat`               — per-core usage deltas
//!   `/proc/cpuinfo`            — CPU name, core count
//!   `/proc/loadavg`            — 1/5/15-min load averages
//!   `/proc/uptime`             — system uptime
//!   `/sys/devices/system/cpu/cpufreq/policy*/scaling_cur_freq` — per-core frequency
//!   `/sys/class/hwmon/hwmon*/` — package + core temperatures (coretemp / k10temp / zenpower)
//!   `/sys/class/powercap/intel-rapl:0/energy_uj` — RAPL power consumption
//!   `/sys/class/power_supply/BAT*` — battery level and status

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use anyhow::Result;
use serde::{Deserialize, Serialize};

// ─── Public types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CpuStats {
    pub usage_percent: f64,
    pub cores_percent: Vec<f64>,
    pub freq_mhz: Vec<u64>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub core_temps_c: Vec<i64>,
    pub package_temp_c: i64,
    pub load_avg: [f64; 3],
    pub uptime_seconds: f64,
    pub power_watts: f64,
    pub cpu_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub battery_percent: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub battery_status: Option<String>,
}

// ─── Internal delta state ─────────────────────────────────────────────────────

#[derive(Clone)]
struct CoreSnapshot {
    active: u64,
    total: u64,
}

struct CpuState {
    prev_deltas: HashMap<String, CoreSnapshot>,
    prev_energy_uj: i64,
    prev_energy_time: Option<Instant>,
}

static CPU_STATE: OnceLock<Mutex<CpuState>> = OnceLock::new();

fn cpu_state() -> &'static Mutex<CpuState> {
    CPU_STATE.get_or_init(|| {
        Mutex::new(CpuState {
            prev_deltas: HashMap::new(),
            prev_energy_uj: 0,
            prev_energy_time: None,
        })
    })
}

// ─── Main collector ───────────────────────────────────────────────────────────

/// Collects a full CPU snapshot. The first call returns zero-percent values
/// because per-core usage requires two samples to compute a delta (same as btop).
pub fn collect() -> Result<CpuStats> {
    let mut stats = CpuStats::default();

    collect_uptime(&mut stats);
    collect_loadavg(&mut stats);
    collect_cpu_name(&mut stats);
    collect_usage(&mut stats);
    collect_frequencies(&mut stats);
    collect_temperatures(&mut stats);
    collect_power(&mut stats);
    collect_battery(&mut stats);

    Ok(stats)
}

// ─── Individual collectors ────────────────────────────────────────────────────

fn collect_uptime(stats: &mut CpuStats) {
    let Ok(content) = std::fs::read_to_string("/proc/uptime") else { return };
    if let Some(token) = content.split_whitespace().next() {
        stats.uptime_seconds = token.parse().unwrap_or(0.0);
    }
}

fn collect_loadavg(stats: &mut CpuStats) {
    let Ok(content) = std::fs::read_to_string("/proc/loadavg") else { return };
    let mut it = content.split_whitespace();
    stats.load_avg[0] = it.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    stats.load_avg[1] = it.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    stats.load_avg[2] = it.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
}

fn collect_cpu_name(stats: &mut CpuStats) {
    static CPU_NAME: OnceLock<String> = OnceLock::new();
    let name = CPU_NAME.get_or_init(|| {
        if let Ok(content) = std::fs::read_to_string("/proc/cpuinfo") {
            for line in content.lines() {
                if let Some(rest) = line.strip_prefix("model name") {
                    if let Some(n) = rest.splitn(2, ':').nth(1) {
                        return n.trim().to_string();
                    }
                }
            }
        }
        "Unknown".to_string()
    });
    stats.cpu_name = name.clone();
}

/// Reads `/proc/stat` and computes per-core and aggregate usage via delta against
/// the previous snapshot — identical to btop's `Cpu::collect()` logic.
fn collect_usage(stats: &mut CpuStats) {
    let Ok(content) = std::fs::read_to_string("/proc/stat") else { return };
    let mut state = cpu_state().lock().unwrap();

    for line in content.lines() {
        if !line.starts_with("cpu") {
            break;
        }
        let mut fields = line.split_whitespace();
        let id = fields.next().unwrap_or("").to_string();

        let vals: Vec<u64> = fields
            .map(|f| f.parse::<u64>().unwrap_or(0))
            .collect();
        if vals.is_empty() { continue; }

        // btop: totals = sum(all) - sum(vals[8..]) (subtract guest fields)
        let total_sum: u64 = vals.iter().sum();
        let guest_sum: u64 = vals.get(8..).unwrap_or(&[]).iter().sum();
        let totals = total_sum - guest_sum;

        // btop: idles = idle(3) + iowait(4)
        let idles = vals.get(3).copied().unwrap_or(0)
            + vals.get(4).copied().unwrap_or(0);
        let active = totals.saturating_sub(idles);

        let curr = CoreSnapshot { active, total: totals };
        let percent = if let Some(prev) = state.prev_deltas.get(&id) {
            let delta_total = curr.total.saturating_sub(prev.total) as f64;
            if delta_total > 0.0 {
                let delta_active = curr.active.saturating_sub(prev.active) as f64;
                (delta_active / delta_total * 100.0).clamp(0.0, 100.0)
            } else {
                0.0
            }
        } else {
            0.0
        };

        state.prev_deltas.insert(id.clone(), curr);

        if id == "cpu" {
            stats.usage_percent = percent;
        } else {
            stats.cores_percent.push(percent);
        }
    }
}

fn collect_frequencies(stats: &mut CpuStats) {
    static FREQ_PATHS: OnceLock<Vec<std::path::PathBuf>> = OnceLock::new();
    let paths = FREQ_PATHS.get_or_init(|| {
        let pattern = "/sys/devices/system/cpu/cpufreq/policy*/scaling_cur_freq";
        let Ok(paths) = glob::glob(pattern) else { return Vec::new() };
        let mut p: Vec<(usize, std::path::PathBuf)> = paths
            .filter_map(|res| res.ok())
            .filter_map(|path| {
                let idx: usize = path.parent()?.file_name()?.to_str()?.strip_prefix("policy")?.parse().ok()?;
                Some((idx, path))
            })
            .collect();
        p.sort_by_key(|(idx, _)| *idx);
        p.into_iter().map(|(_, path)| path).collect()
    });

    stats.freq_mhz = paths.iter()
        .filter_map(|p| {
            std::fs::read_to_string(p).ok()?.trim().parse::<u64>().ok().map(|raw| raw / 1000)
        })
        .collect();
}

/// Scans hwmon entries for coretemp / k10temp / zenpower sensors.
/// Falls back to `/sys/devices/platform/coretemp.0/hwmon` then
/// `/sys/class/thermal/thermal_zone*` — same chain as btop.
fn collect_temperatures(stats: &mut CpuStats) {
    if try_hwmon_temps(stats) { return; }
    if try_platform_coretemp(stats) { return; }
    try_thermal_zones(stats);
}

fn try_hwmon_temps(stats: &mut CpuStats) -> bool {
    let Ok(paths) = glob::glob("/sys/class/hwmon/hwmon*") else { return false };
    let mut found_package = false;

    for path in paths.flatten() {
        let name_path = path.join("name");
        let Ok(name) = std::fs::read_to_string(&name_path) else { continue };
        let name = name.trim();
        if !matches!(name, "coretemp" | "k10temp" | "zenpower") { continue; }

        // Collect all sensor paths first, then sort numerically so we read
        // them in a stable order (temp1, temp2, … temp16, etc.)
        let Ok(temp_inputs) = glob::glob(&format!("{}/temp*_input", path.display())) else { continue };
        let mut sensor_paths: Vec<std::path::PathBuf> = temp_inputs.flatten().collect();
        sensor_paths.sort();

        for tp in sensor_paths {
            let label_path = tp.with_extension("").with_file_name(
                tp.file_name()
                    .unwrap_or_default()
                    .to_str()
                    .unwrap_or("")
                    .replace("input", "label"),
            );
            let label = std::fs::read_to_string(&label_path)
                .unwrap_or_default()
                .trim()
                .to_lowercase();
            let Ok(raw) = std::fs::read_to_string(&tp) else { continue };
            let val: i64 = raw.trim().parse().unwrap_or(0) / 1000;

            if label.contains("package") || label.contains("tdie") || label.contains("tctl") {
                // Record the package/die temp but keep scanning for core temps
                stats.package_temp_c = val;
                found_package = true;
            } else if label.contains("core") || label.contains("tccd") {
                stats.core_temps_c.push(val);
            }
        }
    }

    found_package || !stats.core_temps_c.is_empty()
}

fn try_platform_coretemp(stats: &mut CpuStats) -> bool {
    let Ok(paths) = glob::glob("/sys/devices/platform/coretemp.0/hwmon/hwmon*/temp*_input") else {
        return false;
    };
    let mut found = false;
    for tp in paths.flatten() {
        let label_file = tp.to_str().unwrap_or("").replace("input", "label");
        let label = std::fs::read_to_string(&label_file)
            .unwrap_or_default()
            .trim()
            .to_lowercase();
        let Ok(raw) = std::fs::read_to_string(&tp) else { continue };
        let val: i64 = raw.trim().parse().unwrap_or(0) / 1000;
        if label.contains("package") {
            stats.package_temp_c = val;
            found = true;
        } else if label.contains("core") {
            stats.core_temps_c.push(val);
        }
    }
    found
}

fn try_thermal_zones(stats: &mut CpuStats) {
    let Ok(zones) = glob::glob("/sys/class/thermal/thermal_zone*/temp") else { return };
    for zone in zones.flatten() {
        let Ok(raw) = std::fs::read_to_string(&zone) else { continue };
        let val: i64 = raw.trim().parse().unwrap_or(0) / 1000;
        if val > stats.package_temp_c {
            stats.package_temp_c = val;
        }
    }
}

/// Intel RAPL energy delta → Watts.
fn collect_power(stats: &mut CpuStats) {
    let Ok(raw) = std::fs::read_to_string("/sys/class/powercap/intel-rapl:0/energy_uj") else { return };
    let Ok(curr_uj) = raw.trim().parse::<i64>() else { return };
    let now = Instant::now();
    let mut state = cpu_state().lock().unwrap();
    if state.prev_energy_uj > 0 {
        if let Some(prev_time) = state.prev_energy_time {
            let delta_uj = (curr_uj - state.prev_energy_uj) as f64;
            let delta_us = now.duration_since(prev_time).as_micros() as f64;
            if delta_us > 0.0 {
                // μJ / μs = W
                stats.power_watts = delta_uj / delta_us;
            }
        }
    }
    state.prev_energy_uj = curr_uj;
    state.prev_energy_time = Some(now);
}

fn collect_battery(stats: &mut CpuStats) {
    let Ok(paths) = glob::glob("/sys/class/power_supply/BAT*") else { return };
    let Some(bat) = paths.flatten().next() else { return };

    let capacity = std::fs::read_to_string(bat.join("capacity"))
        .ok()
        .and_then(|s| s.trim().parse::<i32>().ok());
    let status = std::fs::read_to_string(bat.join("status"))
        .ok()
        .map(|s| s.trim().to_string());

    stats.battery_percent = capacity;
    stats.battery_status = status;
}

// ─── Field filter (used by CLI get and agent collect_all) ─────────────────────

/// Zeroes out any fields not listed in `fields`.
/// Valid values: `"usage"`, `"freq"`, `"temp"`, `"power"`, `"loadavg"`, `"uptime"`, `"name"`, `"battery"`.
pub fn filter_fields(mut stats: CpuStats, fields: &[String]) -> CpuStats {
    let set: std::collections::HashSet<&str> = fields.iter().map(String::as_str).collect();
    if !set.contains("usage")   { stats.usage_percent = 0.0; stats.cores_percent.clear(); }
    if !set.contains("freq")    { stats.freq_mhz.clear(); }
    if !set.contains("temp")    { stats.core_temps_c.clear(); stats.package_temp_c = 0; }
    if !set.contains("power")   { stats.power_watts = 0.0; }
    if !set.contains("loadavg") { stats.load_avg = [0.0; 3]; }
    if !set.contains("uptime")  { stats.uptime_seconds = 0.0; }
    if !set.contains("name")    { stats.cpu_name.clear(); }
    if !set.contains("battery") { stats.battery_percent = None; stats.battery_status = None; }
    stats
}

