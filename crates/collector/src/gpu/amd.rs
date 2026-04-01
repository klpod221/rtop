//! AMD GPU collector via sysfs.
//!
//! Data sources: `/sys/class/drm/card*/device/`
//!   `gpu_busy_percent`   — utilization
//!   `mem_info_vram_*`    — VRAM usage
//!   `hwmon/hwmon*/temp1_input` — temperature
//!   `hwmon/hwmon*/power1_average` — power
//!   `pp_dpm_sclk`        — current freq

use super::AmdGpuStats;

/// Returns stats for all detected AMD GPUs.
pub fn collect() -> Vec<AmdGpuStats> {
    let Ok(cards) = glob::glob("/sys/class/drm/card*") else { return Vec::new() };
    let mut results = Vec::new();

    for card_path in cards.flatten() {
        let dev = card_path.join("device");
        if !dev.exists() { continue; }

        // Check vendor id — AMD is 0x1002
        let vendor_id = std::fs::read_to_string(dev.join("vendor"))
            .unwrap_or_default();
        if vendor_id.trim() != "0x1002" { continue; }

        let usage: f64 = std::fs::read_to_string(dev.join("gpu_busy_percent"))
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0.0);

        let mem_used: u64 = std::fs::read_to_string(dev.join("mem_info_vram_used"))
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);

        let mem_total: u64 = std::fs::read_to_string(dev.join("mem_info_vram_total"))
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);

        // Temperature via hwmon
        let mut temp_c: f64 = 0.0;
        if let Ok(hwmons) = glob::glob(&format!("{}/hwmon/hwmon*/temp1_input", dev.display())) {
            for hwmon in hwmons.flatten() {
                if let Ok(raw) = std::fs::read_to_string(&hwmon) {
                    temp_c = raw.trim().parse::<f64>().unwrap_or(0.0) / 1000.0;
                    break;
                }
            }
        }

        // Power via hwmon
        let mut power_watts: f64 = 0.0;
        if let Ok(hwmons) = glob::glob(&format!("{}/hwmon/hwmon*/power1_average", dev.display())) {
            for hwmon in hwmons.flatten() {
                if let Ok(raw) = std::fs::read_to_string(&hwmon) {
                    // Value is in microwatts
                    power_watts = raw.trim().parse::<f64>().unwrap_or(0.0) / 1_000_000.0;
                    break;
                }
            }
        }

        // Current GPU clock from pp_dpm_sclk (active line marked with *)
        let mut freq_mhz: u64 = 0;
        if let Ok(sclk) = std::fs::read_to_string(dev.join("pp_dpm_sclk")) {
            for line in sclk.lines() {
                if line.ends_with('*') {
                    // Format: "0: 300Mhz *" or "1: 2100Mhz *"
                    if let Some(mhz_str) = line.split_whitespace().nth(1) {
                        freq_mhz = mhz_str
                            .trim_end_matches("Mhz")
                            .trim_end_matches("MHz")
                            .parse()
                            .unwrap_or(0);
                    }
                    break;
                }
            }
        }

        let name = read_gpu_name(&card_path);
        results.push(AmdGpuStats { name, usage_percent: usage, mem_used, mem_total, temp_c, power_watts, freq_mhz });
    }
    results
}

/// Returns display names of all AMD GPUs.
pub fn gpu_names() -> Vec<String> {
    let Ok(cards) = glob::glob("/sys/class/drm/card*") else { return Vec::new() };
    cards.flatten()
        .filter(|p| {
            let vendor = std::fs::read_to_string(p.join("device/vendor")).unwrap_or_default();
            vendor.trim() == "0x1002"
        })
        .map(|p| read_gpu_name(&p))
        .filter(|n| !n.is_empty())
        .collect()
}

fn read_gpu_name(card_path: &std::path::Path) -> String {
    // Try DRM label first, then product_name
    for rel in &["device/label", "device/product_name"] {
        if let Ok(name) = std::fs::read_to_string(card_path.join(rel)) {
            let name = name.trim();
            if !name.is_empty() { return name.to_string(); }
        }
    }
    // Fallback: card name from path
    card_path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("AMD GPU")
        .to_string()
}
