//! NVIDIA GPU collector using `nvml-wrapper`.
//!
//! `nvml-wrapper` loads `libnvidia-ml.so` dynamically at runtime via `libloading`.
//! This means the binary compiles and runs on any system — if NVIDIA drivers are
//! absent, `Nvml::init()` returns an error and the GPU module is silently disabled.

use std::sync::OnceLock;

use anyhow::Result;
use nvml_wrapper::Nvml;

use super::NvidiaGpuStats;

static NVML: OnceLock<Option<Nvml>> = OnceLock::new();

/// Initialises NVML (called once at program start). Errors are silently swallowed
/// so that machines without NVIDIA drivers work normally.
pub fn init() {
    NVML.get_or_init(|| Nvml::init().ok());
}

/// Returns stats for all NVIDIA GPUs. Returns an empty vec if NVML is unavailable.
pub fn collect() -> Vec<NvidiaGpuStats> {
    let Some(nvml) = NVML.get().and_then(|o| o.as_ref()) else {
        return Vec::new();
    };
    let Ok(count) = nvml.device_count() else { return Vec::new() };

    (0..count).filter_map(|i| collect_one(nvml, i)).collect()
}

/// Returns display names of all NVIDIA GPUs.
pub fn gpu_names() -> Result<Vec<String>> {
    let Some(nvml) = NVML.get().and_then(|o| o.as_ref()) else {
        return Ok(Vec::new());
    };
    let count = nvml.device_count()?;
    Ok((0..count)
        .filter_map(|i| {
            let dev = nvml.device_by_index(i).ok()?;
            dev.name().ok()
        })
        .collect())
}

fn collect_one(nvml: &Nvml, index: u32) -> Option<NvidiaGpuStats> {
    let dev = nvml.device_by_index(index).ok()?;

    let name = dev.name().unwrap_or_else(|_| format!("NVIDIA GPU {index}"));
    let usage = dev.utilization_rates().ok();
    let mem   = dev.memory_info().ok();
    let temp  = dev.temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu).ok();
    let power = dev.power_usage().ok(); // milliwatts
    let freq  = dev.clock_info(nvml_wrapper::enum_wrappers::device::Clock::Graphics).ok();
    let fan   = dev.fan_speed(0).ok(); // percent, may fail on some cards

    Some(NvidiaGpuStats {
        index,
        name,
        usage_percent: usage.map(|u| u.gpu as f64).unwrap_or(0.0),
        mem_used:  mem.as_ref().map(|m| m.used).unwrap_or(0),
        mem_total: mem.as_ref().map(|m| m.total).unwrap_or(0),
        temp_c:    temp.map(|t| t as i32).unwrap_or(0),
        power_watts: power.map(|p| p as f64 / 1000.0).unwrap_or(0.0),
        freq_mhz:  freq.unwrap_or(0),
        fan_percent: fan,
    })
}
