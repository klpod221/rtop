//! System / host information collector.
//!
//! Data sources:
//!   `/sys/devices/virtual/dmi/id/` — motherboard, vendor (DMI)
//!   `libc::uname()`                — kernel version
//!   `/etc/os-release`              — distro name and version
//!   `/proc/cpuinfo`                — CPU model names
//!   `/sys/bus/pci/devices/`        — GPU detection
//!   `/proc/meminfo` + dmidecode    — RAM
//!   `/sys/block/`                  — disk models

use std::collections::HashSet;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HostInfo {
    pub os_vendor: String,
    pub os_version: String,
    pub kernel_version: String,
    pub system_vendor: String,
    pub motherboard_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_family: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub cpus: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub gpus: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub physical_ram: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub disk_models: Vec<String>,
}

// Collected once and cached for the lifetime of the process (static hardware info).
static CACHED: OnceLock<HostInfo> = OnceLock::new();

/// Returns static host information (cached after first call).
pub fn collect() -> HostInfo {
    CACHED.get_or_init(collect_once).clone()
}

fn collect_once() -> HostInfo {
    let mut info = HostInfo::default();
    collect_dmi(&mut info);
    collect_kernel(&mut info);
    collect_os_release(&mut info);
    collect_cpus(&mut info);
    collect_gpus(&mut info);
    collect_ram(&mut info);
    collect_disks(&mut info);
    info
}

fn read_dmi(field: &str) -> String {
    std::fs::read_to_string(format!("/sys/devices/virtual/dmi/id/{field}"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| {
            !matches!(
                s.as_str(),
                "Default string" | "To be filled by O.E.M." | "System Product Name" | ""
            )
        })
        .unwrap_or_default()
}

fn collect_dmi(info: &mut HostInfo) {
    info.system_vendor = read_dmi("sys_vendor");
    if info.system_vendor.is_empty() {
        info.system_vendor = read_dmi("board_vendor");
    }
    info.motherboard_name = read_dmi("board_name");
    if info.motherboard_name.is_empty() {
        info.motherboard_name = read_dmi("product_name");
    }
    let family = read_dmi("product_family");
    info.product_family = if family.is_empty() { None } else { Some(family) };
}

fn collect_kernel(info: &mut HostInfo) {
    let mut buf: libc::utsname = unsafe { std::mem::zeroed() };
    // SAFETY: utsname is correctly sized; uname fills it.
    if unsafe { libc::uname(&mut buf) } == 0 {
        info.kernel_version = c_chars_to_string(&buf.release);
    }
}

fn c_chars_to_string(chars: &[libc::c_char]) -> String {
    chars.iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8 as char)
        .collect()
}

fn collect_os_release(info: &mut HostInfo) {
    let Ok(content) = std::fs::read_to_string("/etc/os-release") else { return };
    for line in content.lines() {
        if let Some(v) = line.strip_prefix("PRETTY_NAME=") {
            info.os_vendor = v.trim_matches('"').trim_matches('\'').to_string();
        } else if line.starts_with("NAME=") && info.os_vendor.is_empty() {
            let v = line.trim_start_matches("NAME=");
            info.os_vendor = v.trim_matches('"').trim_matches('\'').to_string();
        } else if let Some(v) = line.strip_prefix("VERSION=") {
            info.os_version = v.trim_matches('"').trim_matches('\'').to_string();
        } else if let Some(v) = line.strip_prefix("BUILD_ID=") {
            if info.os_version.is_empty() {
                info.os_version = v.trim_matches('"').trim_matches('\'').to_string();
            }
        }
    }
}

fn collect_cpus(info: &mut HostInfo) {
    let Ok(content) = std::fs::read_to_string("/proc/cpuinfo") else { return };
    let mut seen = HashSet::new();
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("model name") {
            if let Some(name) = rest.splitn(2, ':').nth(1) {
                let name = name.trim().to_string();
                if seen.insert(name.clone()) {
                    info.cpus.push(name);
                }
            }
        }
    }
}

fn collect_gpus(info: &mut HostInfo) {
    info.gpus = super::gpu::all_gpu_names();
}

fn collect_ram(info: &mut HostInfo) {
    info.physical_ram = super::mem::collect()
        .ok()
        .map(|m| m.physical_ram)
        .unwrap_or_default();

    if info.physical_ram.is_empty() {
        // Fallback: report MemTotal from /proc/meminfo
        if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
            for line in content.lines() {
                if line.starts_with("MemTotal:") {
                    let kb: u64 = line.split_whitespace()
                        .nth(1)
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0);
                    let gb = kb as f64 / 1024.0 / 1024.0;
                    info.physical_ram.push(format!("{:.2} GiB Total", gb));
                    break;
                }
            }
        }
    }
}

fn collect_disks(info: &mut HostInfo) {
    let Ok(entries) = std::fs::read_dir("/sys/block") else { return };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("loop") || name.starts_with("ram")
            || name.starts_with("dm-") || name.starts_with("sr")
        {
            continue;
        }
        let model_path = format!("/sys/block/{name}/device/model");
        if let Ok(model) = std::fs::read_to_string(&model_path) {
            let model = model.trim().to_string();
            if !model.is_empty() {
                info.disk_models.push(model);
            }
        }
    }
}
