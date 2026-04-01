//! GPU collectors — Intel (PMU), NVIDIA (NVML), AMD (sysfs).

pub mod amd;
pub mod intel;
#[cfg(feature = "nvidia")]
pub mod nvidia;

use serde::{Deserialize, Serialize};

// ─── Shared public types ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EngineUsage {
    pub name: String,
    pub busy_pct: f64,
}

/// Intel GPU stats collected via Linux perf PMU.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IntelGpuStats {
    pub name: String,
    pub engines: Vec<EngineUsage>,
    pub freq_act_mhz: f64,
    pub freq_req_mhz: f64,
    pub rc6_pct: f64,
    pub power_gpu_watts: f64,
    pub power_pkg_watts: f64,
    pub imc_reads_mbs: f64,
    pub imc_writes_mbs: f64,
}

/// NVIDIA GPU stats collected via NVML.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NvidiaGpuStats {
    pub index: u32,
    pub name: String,
    pub usage_percent: f64,
    pub mem_used: u64,
    pub mem_total: u64,
    pub temp_c: i32,
    pub power_watts: f64,
    pub freq_mhz: u32,
    pub fan_percent: Option<u32>,
}

/// AMD GPU stats collected via sysfs.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AmdGpuStats {
    pub name: String,
    pub usage_percent: f64,
    pub mem_used: u64,
    pub mem_total: u64,
    pub temp_c: f64,
    pub power_watts: f64,
    pub freq_mhz: u64,
}

// ─── Name helpers (used by system.rs) ────────────────────────────────────────

/// Returns display names for all detected GPUs across all vendors.
pub fn all_gpu_names() -> Vec<String> {
    let mut names: Vec<String> = Vec::new();

    // Intel
    if let Ok(name) = intel_gpu_name() {
        if !name.is_empty() { names.push(name); }
    }

    // NVIDIA
    #[cfg(feature = "nvidia")]
    {
        if let Ok(nv_names) = nvidia::gpu_names() {
            names.extend(nv_names);
        }
    }

    // AMD
    names.extend(amd::gpu_names());

    names
}

fn intel_gpu_name() -> anyhow::Result<String> {
    // Try reading from PCI subsystem for i915/xe devices
    for pattern in &[
        "/sys/bus/pci/drivers/i915/*/device/label",
        "/sys/bus/pci/drivers/xe/*/device/label",
    ] {
        if let Ok(paths) = glob::glob(pattern) {
            for path in paths.flatten() {
                if let Ok(label) = std::fs::read_to_string(&path) {
                    return Ok(label.trim().to_string());
                }
            }
        }
    }
    // Fallback: modalias-based name from DRM
    for pattern in &[
        "/sys/class/drm/card*/device/product_name",
        "/sys/class/drm/renderD*/device/product_name",
    ] {
        if let Ok(paths) = glob::glob(pattern) {
            for path in paths.flatten() {
                if let Ok(name) = std::fs::read_to_string(&path) {
                    let name = name.trim();
                    if !name.is_empty() {
                        return Ok(name.to_string());
                    }
                }
            }
        }
    }
    Ok(String::new())
}
