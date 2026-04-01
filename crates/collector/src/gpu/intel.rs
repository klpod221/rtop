//! Intel GPU collector via Linux perf PMU.
//!
//! This is a direct port of btop's `pmu_init()` / `pmu_sample()` / `pmu_calc()`
//! from `src/linux/btop_gpu.cpp`, using raw `perf_event_open(2)` syscalls via libc.
//!
//! Requires `CAP_PERFMON` (or `cap_perfmon,cap_dac_read_search=ep`) on the binary.
//! Without it, `IntelGpuCollector::new()` returns an error and GPU collection is skipped.

use std::os::unix::io::RawFd;

use anyhow::{bail, Context, Result};

use super::IntelGpuStats;
use super::EngineUsage;

// Linux perf_event_attr — subset needed for perf_event_open.
// The real struct is 128 bytes; we allocate that size precisely.
// Fields not listed here are zeroed by `std::mem::zeroed()`.
#[repr(C)]
struct PerfEventAttr {
    type_:       u32,
    size:        u32,
    config:      u64,
    _sample_period_or_freq: u64,
    sample_type: u64,
    read_format: u64,
    _bitfield_1: u64,  // disabled:1, inherit:1, pinned:1, exclusive:1, …, use_clockid:1, …
    _bitfield_2: u32,
    clockid:     i32,
    // Total struct is 128 bytes; pad the rest.
    _pad: [u64; 9],
}

impl PerfEventAttr {
    /// Zeroed (safe) constructor.
    fn zeroed() -> Self { unsafe { std::mem::zeroed() } }

    /// Sets the `use_clockid` bit (bit 24 in the first bitfield u64).
    fn set_use_clockid(&mut self) {
        self._bitfield_1 |= 1u64 << 24;
    }
}

// ─── perf_event_open constants ────────────────────────────────────────────────

const PERF_FORMAT_TOTAL_TIME_ENABLED: u64 = 1 << 1;
const PERF_FORMAT_GROUP: u64              = 1 << 3;
#[allow(dead_code)]
const USE_CLOCKID_BIT: u64               = 1 << 24; // flags bit in perf_event_attr

// ─── Internal structures ──────────────────────────────────────────────────────

#[derive(Clone, Default)]
struct PmuCounter {
    config: u64,
    index: usize,
    scale: f64,
    present: bool,
    prev: u64,
    cur: u64,
}

struct EngineInfo {
    name: String,
    busy: PmuCounter,
}

/// Manages open perf event group file descriptors for an Intel GPU.
pub struct IntelGpuCollector {
    /// Main group fd (engine + freq + rc6 group).
    fd: RawFd,
    /// RAPL power group fd.
    rapl_fd: RawFd,
    /// IMC memory bandwidth group fd.
    imc_fd: RawFd,

    num_counters: usize,
    num_rapl: usize,
    num_imc: usize,

    engines: Vec<EngineInfo>,
    freq_req: PmuCounter,
    freq_act: PmuCounter,
    rc6: PmuCounter,
    irq: PmuCounter,
    r_gpu: PmuCounter,
    r_pkg: PmuCounter,
    imc_read: PmuCounter,
    imc_write: PmuCounter,

    ts_prev: u64,
    ts_cur: u64,
}

impl Drop for IntelGpuCollector {
    fn drop(&mut self) {
        if self.fd     >= 0 { unsafe { libc::close(self.fd);     } }
        if self.rapl_fd >= 0 { unsafe { libc::close(self.rapl_fd); } }
        if self.imc_fd  >= 0 { unsafe { libc::close(self.imc_fd);  } }
    }
}

// ─── Constructor ──────────────────────────────────────────────────────────────

impl IntelGpuCollector {
    /// Initialises the Intel GPU PMU collector, replicating btop's `pmu_init()`.
    ///
    /// Returns `Err` if:
    ///   - No Intel GPU PMU device (`i915` / `xe`) exists,
    ///   - `perf_event_open` fails (missing `CAP_PERFMON`),
    ///   - No engine counters discovered.
    pub fn new() -> Result<Self> {
        let (dev_name, pmu_type) = find_pmu_device()?;
        let base = format!("/sys/bus/event_source/devices/{dev_name}");

        let mut col = IntelGpuCollector {
            fd: -1, rapl_fd: -1, imc_fd: -1,
            num_counters: 0, num_rapl: 0, num_imc: 0,
            engines: Vec::new(),
            freq_req: Default::default(), freq_act: Default::default(),
            rc6: Default::default(), irq: Default::default(),
            r_gpu: Default::default(), r_pkg: Default::default(),
            imc_read: Default::default(), imc_write: Default::default(),
            ts_prev: 0, ts_cur: 0,
        };

        // Step 1 — discover engine counters from PMU events directory
        let events_dir = format!("{base}/events");
        let Ok(entries) = std::fs::read_dir(&events_dir) else {
            bail!("cannot read events dir {events_dir}");
        };
        for entry in entries.flatten() {
            let fname = entry.file_name().to_string_lossy().to_string();
            if !fname.ends_with("-busy") { continue; }
            let engine_name = fname.trim_end_matches("-busy").to_string();
            let Ok((config, _)) = parse_event(&base, &fname) else { continue };
            col.engines.push(EngineInfo {
                name: engine_name,
                busy: PmuCounter { config, ..Default::default() },
            });
        }
        if col.engines.is_empty() {
            bail!("no GPU engines discovered");
        }

        // Step 2 — open interrupt counter as group leader (btop uses interrupts)
        if let Ok((irq_cfg, _)) = parse_event(&base, "interrupts") {
            col.irq.config = irq_cfg;
        }
        let fd = perf_open(pmu_type, col.irq.config, -1)
            .context("failed to open group leader (interrupts)")?;
        col.fd = fd;
        col.irq.index   = col.num_counters;
        col.irq.present = true;
        col.num_counters += 1;

        // Step 3 — open freq/rc6 counters into the same group
        for (counter, event_name) in [
            (&mut col.freq_req as *mut PmuCounter, "requested-frequency"),
            (&mut col.freq_act as *mut PmuCounter, "actual-frequency"),
            (&mut col.rc6      as *mut PmuCounter, "rc6-residency"),
        ] {
            if let Ok((cfg, _)) = parse_event(&base, event_name) {
                if perf_open(pmu_type, cfg, col.fd).is_ok() {
                    // SAFETY: single-threaded init, no aliasing
                    let c = unsafe { &mut *counter };
                    c.config  = cfg;
                    c.index   = col.num_counters;
                    c.present = true;
                    col.num_counters += 1;
                }
            }
        }

        // Step 4 — open engine busy counters into the group
        for i in 0..col.engines.len() {
            let cfg = col.engines[i].busy.config;
            if perf_open(pmu_type, cfg, col.fd).is_ok() {
                col.engines[i].busy.index   = col.num_counters;
                col.engines[i].busy.present = true;
                col.num_counters += 1;
            }
        }

        // Step 5 — open RAPL power (separate group)
        let rapl_base = "/sys/devices/power";
        if let Ok(rapl_type) = read_pmu_type(rapl_base) {
            for (counter, event_name) in [
                (&mut col.r_gpu as *mut PmuCounter, "energy-gpu"),
                (&mut col.r_pkg as *mut PmuCounter, "energy-pkg"),
            ] {
                if let Ok((cfg, scale)) = parse_event(rapl_base, event_name) {
                    if let Ok(rfd) = perf_open(rapl_type, cfg, col.rapl_fd) {
                        if col.rapl_fd < 0 { col.rapl_fd = rfd; }
                        let c = unsafe { &mut *counter };
                        c.config  = cfg;
                        c.scale   = scale;
                        c.index   = col.num_rapl;
                        c.present = true;
                        col.num_rapl += 1;
                    }
                }
            }
        }

        // Step 6 — open IMC memory bandwidth (separate group)
        let imc_base = "/sys/devices/uncore_imc";
        if let Ok(imc_type) = read_pmu_type(imc_base) {
            for (counter, event_name) in [
                (&mut col.imc_read  as *mut PmuCounter, "data_reads"),
                (&mut col.imc_write as *mut PmuCounter, "data_writes"),
            ] {
                if let Ok((cfg, scale)) = parse_event(imc_base, event_name) {
                    if let Ok(ifd) = perf_open(imc_type, cfg, col.imc_fd) {
                        if col.imc_fd < 0 { col.imc_fd = ifd; }
                        let c = unsafe { &mut *counter };
                        c.config  = cfg;
                        c.scale   = scale;
                        c.index   = col.num_imc;
                        c.present = true;
                        col.num_imc += 1;
                    }
                }
            }
        }

        Ok(col)
    }
}

// ─── Sample ───────────────────────────────────────────────────────────────────

impl IntelGpuCollector {
    /// Performs one PMU sample cycle.
    ///
    /// The first call stores a baseline; computed values appear from the second
    /// call onwards (same as btop).
    pub fn collect(&mut self) -> IntelGpuStats {
        let mut stats = IntelGpuStats::default();
        if self.fd < 0 { return stats; }

        self.ts_prev = self.ts_cur;

        let Ok((ts, vals)) = pmu_read_multi(self.fd, self.num_counters) else {
            return stats;
        };
        self.ts_cur = ts;

        // Update engine counters
        for eng in &mut self.engines {
            update_sample(&mut eng.busy, &vals);
        }
        update_sample(&mut self.freq_req, &vals);
        update_sample(&mut self.freq_act, &vals);
        update_sample(&mut self.rc6, &vals);

        // RAPL group
        if self.rapl_fd >= 0 && self.num_rapl > 0 {
            if let Ok((_, rapl_vals)) = pmu_read_multi(self.rapl_fd, self.num_rapl) {
                update_sample(&mut self.r_gpu, &rapl_vals);
                update_sample(&mut self.r_pkg, &rapl_vals);
            }
        }

        // IMC group
        if self.imc_fd >= 0 && self.num_imc > 0 {
            if let Ok((_, imc_vals)) = pmu_read_multi(self.imc_fd, self.num_imc) {
                update_sample(&mut self.imc_read, &imc_vals);
                update_sample(&mut self.imc_write, &imc_vals);
            }
        }

        if self.ts_prev == 0 { return stats; }
        let delta_ns = self.ts_cur.saturating_sub(self.ts_prev) as f64;
        if delta_ns <= 0.0 { return stats; }
        let t = delta_ns / 1e9; // seconds

        // Engine utilization: busy_ns / 1e9 / t * 100 = %
        for eng in &self.engines {
            if eng.busy.present {
                let pct = pmu_calc(eng.busy.prev, eng.busy.cur, 1e9, t, 100.0);
                stats.engines.push(EngineUsage { name: eng.name.clone(), busy_pct: pct });
            }
        }

        if self.freq_act.present {
            stats.freq_act_mhz = pmu_calc(self.freq_act.prev, self.freq_act.cur, 1.0, t, 1.0);
        }
        if self.freq_req.present {
            stats.freq_req_mhz = pmu_calc(self.freq_req.prev, self.freq_req.cur, 1.0, t, 1.0);
        }
        if self.rc6.present {
            stats.rc6_pct = pmu_calc(self.rc6.prev, self.rc6.cur, 1e9, t, 100.0);
        }
        if self.r_gpu.present && self.r_gpu.scale != 0.0 {
            stats.power_gpu_watts = pmu_calc(self.r_gpu.prev, self.r_gpu.cur, 1.0, t, self.r_gpu.scale);
        }
        if self.r_pkg.present && self.r_pkg.scale != 0.0 {
            stats.power_pkg_watts = pmu_calc(self.r_pkg.prev, self.r_pkg.cur, 1.0, t, self.r_pkg.scale);
        }
        if self.imc_read.present && self.imc_read.scale != 0.0 {
            stats.imc_reads_mbs = pmu_calc(self.imc_read.prev, self.imc_read.cur, 1.0, t, self.imc_read.scale) / (1024.0 * 1024.0);
        }
        if self.imc_write.present && self.imc_write.scale != 0.0 {
            stats.imc_writes_mbs = pmu_calc(self.imc_write.prev, self.imc_write.cur, 1.0, t, self.imc_write.scale) / (1024.0 * 1024.0);
        }

        stats
    }
}

// ─── Low-level perf helpers ───────────────────────────────────────────────────

/// Finds the first available Intel GPU PMU device (`i915` or `xe`).
fn find_pmu_device() -> Result<(String, u64)> {
    for dev in &["i915", "xe"] {
        let path = format!("/sys/bus/event_source/devices/{dev}/type");
        if let Ok(raw) = std::fs::read_to_string(&path) {
            if let Ok(t) = raw.trim().parse::<u64>() {
                if t > 0 { return Ok((dev.to_string(), t)); }
            }
        }
    }
    bail!("no Intel GPU PMU device found")
}

fn read_pmu_type(base: &str) -> Result<u64> {
    let raw = std::fs::read_to_string(format!("{base}/type"))?;
    Ok(raw.trim().parse()?)
}

/// Reads `config=0xNN` and optional `scale` for a named event.
fn parse_event(base: &str, event_name: &str) -> Result<(u64, f64)> {
    let event_path = format!("{base}/events/{event_name}");
    let content = std::fs::read_to_string(&event_path)
        .with_context(|| format!("reading event {event_path}"))?;
    let content = content.trim();

    let config = if let Some(rest) = content.strip_prefix("config=") {
        u64::from_str_radix(rest.trim_start_matches("0x"), 16).unwrap_or(0)
    } else if let Some(rest) = content.strip_prefix("event=") {
        u64::from_str_radix(rest.trim_start_matches("0x"), 16).unwrap_or(0)
    } else { 0 };

    let scale: f64 = std::fs::read_to_string(format!("{base}/events/{event_name}.scale"))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(1.0);

    Ok((config, scale))
}

/// Opens a perf event, trying CPUs 0..128 until one succeeds (btop's `_perf_open()`).
fn perf_open(pmu_type: u64, config: u64, group_fd: RawFd) -> Result<RawFd> {
    // Build perf_event_attr on the stack (128-byte struct, must be zeroed)
    let mut attr = PerfEventAttr::zeroed();
    attr.type_       = pmu_type as u32;
    attr.size        = std::mem::size_of::<PerfEventAttr>() as u32;
    attr.config      = config;
    attr.set_use_clockid();
    attr.clockid     = libc::CLOCK_MONOTONIC;
    attr.read_format = if group_fd == -1 {
        PERF_FORMAT_TOTAL_TIME_ENABLED | PERF_FORMAT_GROUP
    } else {
        PERF_FORMAT_TOTAL_TIME_ENABLED
    };

    for cpu in 0i32..128 {
        // SAFETY: attr is a correctly-initialised 128-byte struct; pid=-1 means any process.
        let fd = unsafe {
            libc::syscall(
                libc::SYS_perf_event_open,
                &attr as *const PerfEventAttr,
                -1i32,   // pid: any
                cpu,
                group_fd,
                0u64,    // flags
            ) as RawFd
        };
        if fd >= 0 { return Ok(fd); }
        let err = unsafe { *libc::__errno_location() };
        if err != libc::EINVAL { break; }
    }
    bail!("perf_event_open failed for config 0x{config:x}")
}

/// Reads grouped perf event counters.
/// Format: `[nr: u64, time_enabled: u64, val0: u64, val1: u64, ...]`
fn pmu_read_multi(fd: RawFd, num_counters: usize) -> Result<(u64, Vec<u64>)> {
    let buf_size = (2 + num_counters) * 8;
    let mut buf = vec![0u8; buf_size];
    // SAFETY: fd is a valid perf group fd; buf is correctly sized.
    let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf_size) };
    if n < buf_size as isize {
        bail!("pmu read failed (got {n}/{buf_size} bytes)");
    }
    let time_enabled = u64::from_le_bytes(buf[8..16].try_into().unwrap());
    let vals: Vec<u64> = (0..num_counters)
        .map(|i| {
            let off = (2 + i) * 8;
            u64::from_le_bytes(buf[off..off + 8].try_into().unwrap())
        })
        .collect();
    Ok((time_enabled, vals))
}

fn update_sample(c: &mut PmuCounter, vals: &[u64]) {
    if c.present && c.index < vals.len() {
        c.prev = c.cur;
        c.cur  = vals[c.index];
    }
}

/// Replicates btop's `pmu_calc(prev, cur, delta_ns_scale, time_secs, result_scale)`.
fn pmu_calc(prev: u64, cur: u64, delta_scale: f64, time_secs: f64, result_scale: f64) -> f64 {
    if time_secs <= 0.0 { return 0.0; }
    let v = (cur.saturating_sub(prev)) as f64 / delta_scale / time_secs * result_scale;
    if result_scale == 100.0 { v.clamp(0.0, 100.0) } else { v }
}
