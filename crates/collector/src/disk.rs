//! Disk metrics collector.
//!
//! Data sources:
//!   `libc::statvfs`        — per-mount disk space
//!   `/proc/diskstats`      — block device IO (read/write bytes delta)
//!   `/proc/mounts`         — mounted filesystems

use std::collections::HashMap;
use std::ffi::CString;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use serde::{Deserialize, Serialize};

// ─── Public types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskSpace {
    pub device: String,
    pub mount_point: String,
    pub fs_type: String,
    pub total: u64,
    pub used: u64,
    pub free: u64,
    pub used_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskIo {
    pub device: String,
    /// Bytes read since last call.
    pub read_bytes_per_sec: f64,
    /// Bytes written since last call.
    pub write_bytes_per_sec: f64,
    pub read_bytes_total: u64,
    pub write_bytes_total: u64,
}

// ─── Delta state ──────────────────────────────────────────────────────────────

struct DiskSnapshot {
    read_sectors: u64,
    write_sectors: u64,
    when: Instant,
}

static DISK_STATE: OnceLock<Mutex<HashMap<String, DiskSnapshot>>> = OnceLock::new();

fn disk_state() -> &'static Mutex<HashMap<String, DiskSnapshot>> {
    DISK_STATE.get_or_init(|| Mutex::new(HashMap::new()))
}

// ─── Collectors ───────────────────────────────────────────────────────────────

/// Returns space stats for all real mounted filesystems.
pub fn collect_space(mount_filter: &[String]) -> Vec<DiskSpace> {
    let mounts = read_mounts();
    let mut results = Vec::new();

    for (device, mount, fstype) in &mounts {
        // Skip pseudo-filesystems
        if is_virtual_fs(fstype) { continue; }
        // Apply optional mount-point filter
        if !mount_filter.is_empty() && !mount_filter.iter().any(|f| f == mount) {
            continue;
        }

        let Ok(cpath) = CString::new(mount.as_str()) else { continue };
        let mut vfs: libc::statvfs64 = unsafe { std::mem::zeroed() };
        // SAFETY: cpath is a valid NUL-terminated path; vfs is correctly-sized.
        if unsafe { libc::statvfs64(cpath.as_ptr(), &mut vfs) } != 0 { continue; }

        let block = vfs.f_bsize as u64;
        let total = vfs.f_blocks * block;
        
        // Skip mounts with 0 capacity (often pseudo-filesystems or unmounted CD-ROMs)
        if total == 0 { continue; }

        let free  = vfs.f_bavail * block;
        let used  = total.saturating_sub(vfs.f_bfree * block);
        let used_percent = used as f64 / total as f64 * 100.0;

        results.push(DiskSpace {
            device: device.clone(),
            mount_point: mount.clone(),
            fs_type: fstype.clone(),
            total,
            used,
            free,
            used_percent,
        });
    }
    results
}

/// Returns per-device IO throughput since the last call.
pub fn collect_io() -> Vec<DiskIo> {
    let Ok(content) = std::fs::read_to_string("/proc/diskstats") else { return Vec::new() };
    let now = Instant::now();
    let mut state = disk_state().lock().unwrap();
    let mut results = Vec::new();

    // Sector size is always 512 bytes per Linux kernel convention.
    const SECTOR_BYTES: u64 = 512;

    for line in content.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        // diskstats format: major minor name reads ... read_sectors ... writes ... write_sectors ...
        if fields.len() < 14 { continue; }
        let name = fields[2];
        // Skip partition entries (keep whole disks: sda, nvme0n1, vda, etc.)
        if name.chars().last().map(|c| c.is_ascii_digit()).unwrap_or(false)
            && (name.starts_with("sd") || name.starts_with("hd") || name.starts_with("vd"))
        {
            continue;
        }
        let read_sectors: u64  = fields[5].parse().unwrap_or(0);
        let write_sectors: u64 = fields[9].parse().unwrap_or(0);

        let (read_bps, write_bps) = if let Some(prev) = state.get(name) {
            let dt = now.duration_since(prev.when).as_secs_f64();
            if dt > 0.0 {
                let dr = read_sectors.saturating_sub(prev.read_sectors) as f64 * SECTOR_BYTES as f64;
                let dw = write_sectors.saturating_sub(prev.write_sectors) as f64 * SECTOR_BYTES as f64;
                (dr / dt, dw / dt)
            } else { (0.0, 0.0) }
        } else { (0.0, 0.0) };

        state.insert(name.to_string(), DiskSnapshot {
            read_sectors,
            write_sectors,
            when: now,
        });

        results.push(DiskIo {
            device: name.to_string(),
            read_bytes_per_sec: read_bps,
            write_bytes_per_sec: write_bps,
            read_bytes_total: read_sectors * SECTOR_BYTES,
            write_bytes_total: write_sectors * SECTOR_BYTES,
        });
    }
    results
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Reads `/proc/mounts` and returns `(device, mountpoint, fstype)` tuples.
fn read_mounts() -> Vec<(String, String, String)> {
    let Ok(content) = std::fs::read_to_string("/proc/mounts") else { return Vec::new() };
    content.lines()
        .filter_map(|line| {
            let mut f = line.split_whitespace();
            let dev   = f.next()?.to_string();
            let mount = f.next()?.to_string();
            let fs    = f.next()?.to_string();
            Some((dev, mount, fs))
        })
        .collect()
}

fn is_virtual_fs(fstype: &str) -> bool {
    matches!(fstype,
        "proc" | "sysfs" | "devtmpfs" | "devpts" | "tmpfs" | "cgroup" | "cgroup2"
        | "pstore" | "efivarfs" | "bpf" | "tracefs" | "debugfs" | "securityfs"
        | "fusectl" | "hugetlbfs" | "mqueue" | "overlay" | "squashfs" | "ramfs"
        | "configfs" | "autofs" | "binfmt_misc" | "nsfs" | "fuse.portal"
        | "rpc_pipefs" | "fuse.lxcfs"
    )
}
