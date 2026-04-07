//! Process list collector.
//!
//! Data sources:
//!   `/proc/[pid]/stat`    — CPU times, state
//!   `/proc/[pid]/status`  — RSS memory
//!   `/proc/[pid]/io`      — cumulative IO bytes (requires CAP_SYS_PTRACE or same-uid)
//!   `/proc/[pid]/cmdline` — full command line
//!   `/etc/passwd`         — UID to username mapping

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use serde::{Deserialize, Serialize};

// ─── Public types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cmdline: String,
    pub state: char,
    pub cpu_percent: f64,
    pub mem_rss_bytes: u64,
    pub mem_percent: f64,
    pub io_read_bytes: u64,
    pub io_write_bytes: u64,
    pub user: String,
    pub threads: u32,
    pub ppid: u32,
}

// ─── Delta state ──────────────────────────────────────────────────────────────

struct ProcSnapshot {
    utime: u64,
    stime: u64,
    when: Instant,
    cmdline: String,
}

static PROC_STATE: OnceLock<Mutex<HashMap<u32, ProcSnapshot>>> = OnceLock::new();

fn proc_state() -> &'static Mutex<HashMap<u32, ProcSnapshot>> {
    PROC_STATE.get_or_init(|| Mutex::new(HashMap::new()))
}

// ─── Collector ────────────────────────────────────────────────────────────────

/// Collects the current process list.
///
/// `total_mem_bytes` is used to compute `mem_percent`.
pub fn collect(total_mem_bytes: u64) -> Vec<ProcessInfo> {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    let now = Instant::now();
    let mut state = proc_state().lock().unwrap();
    let mut results = Vec::new();

    // Fetch Hz from sysconf for CPU time calculation
    let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) } as f64;

    for entry in entries.flatten() {
        let name = entry.file_name();
        let pid_str = name.to_str().unwrap_or("");
        let Ok(pid) = pid_str.parse::<u32>() else {
            continue;
        };

        let Some(info) = collect_one(pid, total_mem_bytes, hz, now, &mut state) else {
            continue;
        };
        results.push(info);
    }

    // Cleanup dead PIDs from cache to avoid memory leak
    let current_pids: std::collections::HashSet<u32> = results.iter().map(|p| p.pid).collect();
    state.retain(|pid, _| current_pids.contains(pid));

    results
}

fn collect_one(
    pid: u32,
    total_mem: u64,
    hz: f64,
    now: Instant,
    state: &mut HashMap<u32, ProcSnapshot>,
) -> Option<ProcessInfo> {
    let stat = read_stat(pid)?;
    let (rss, uid, threads) = read_status(pid);
    let (io_r, io_w) = read_io(pid);

    let utime = stat.utime;
    let stime = stat.stime;

    let (cpu_percent, cmdline) = if let Some(prev) = state.get(&pid) {
        let dt = now.duration_since(prev.when).as_secs_f64();
        let cp = if dt > 0.0 && hz > 0.0 {
            let delta_ticks = (utime + stime).saturating_sub(prev.utime + prev.stime) as f64;
            let cpus = num_cpus().max(1.0);
            ((delta_ticks / hz / dt * 100.0) / cpus).clamp(0.0, 100.0)
        } else {
            0.0
        };
        (cp, prev.cmdline.clone())
    } else {
        (0.0, read_cmdline(pid))
    };

    state.insert(
        pid,
        ProcSnapshot {
            utime,
            stime,
            when: now,
            cmdline: cmdline.clone(),
        },
    );

    let mem_percent = if total_mem > 0 {
        rss as f64 / total_mem as f64 * 100.0
    } else {
        0.0
    };

    Some(ProcessInfo {
        pid,
        name: stat.name,
        cmdline,
        state: stat.state,
        cpu_percent,
        mem_rss_bytes: rss,
        mem_percent,
        io_read_bytes: io_r,
        io_write_bytes: io_w,
        user: resolve_uid(uid),
        threads,
        ppid: stat.ppid,
    })
}

// ─── /proc/[pid]/stat parser ──────────────────────────────────────────────────

struct StatFields {
    name: String,
    state: char,
    ppid: u32,
    utime: u64,
    stime: u64,
}

fn read_stat(pid: u32) -> Option<StatFields> {
    let content = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // Format: pid (name) state ... utime stime ...
    // Name may contain spaces and parentheses, so parse carefully.
    let start = content.find('(')?;
    let end = content.rfind(')')?;
    let name = content[start + 1..end].to_string();
    let rest: Vec<&str> = content[end + 2..].split_whitespace().collect();
    let state = rest.first()?.chars().next().unwrap_or('?');
    // Fields are 0-indexed from the post-name remainder:
    // 0=state 1=ppid 2=pgrp 3=session 4=tty 5=tpgid 6=flags
    // 7=minflt 8=cminflt 9=majflt 10=cmajflt 11=utime 12=stime
    let ppid: u32 = rest.get(1)?.parse().ok()?;
    let utime: u64 = rest.get(11)?.parse().ok()?;
    let stime: u64 = rest.get(12)?.parse().ok()?;
    Some(StatFields {
        name,
        state,
        ppid,
        utime,
        stime,
    })
}

fn read_status(pid: u32) -> (u64, u32, u32) {
    let Ok(content) = std::fs::read_to_string(format!("/proc/{pid}/status")) else {
        return (0, 0, 1);
    };
    let mut rss = 0;
    let mut uid = 0;
    let mut threads = 1;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            rss = rest
                .split_whitespace()
                .next()
                .and_then(|v| v.parse::<u64>().ok())
                .map(|kb| kb << 10)
                .unwrap_or(0);
        } else if let Some(rest) = line.strip_prefix("Uid:") {
            uid = rest
                .split_whitespace()
                .next()
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(0);
        } else if let Some(rest) = line.strip_prefix("Threads:") {
            threads = rest
                .trim()
                .parse::<u32>()
                .unwrap_or(1);
        }
    }
    (rss, uid, threads)
}

fn read_io(pid: u32) -> (u64, u64) {
    let Ok(content) = std::fs::read_to_string(format!("/proc/{pid}/io")) else {
        return (0, 0);
    };
    let (mut r, mut w) = (0u64, 0u64);
    for line in content.lines() {
        if let Some(v) = line.strip_prefix("read_bytes:") {
            r = v.trim().parse().unwrap_or(0);
        } else if let Some(v) = line.strip_prefix("write_bytes:") {
            w = v.trim().parse().unwrap_or(0);
        }
    }
    (r, w)
}

fn read_cmdline(pid: u32) -> String {
    std::fs::read(format!("/proc/{pid}/cmdline"))
        .map(|bytes| {
            bytes
                .split(|&b| b == 0)
                .filter(|s| !s.is_empty())
                .map(|s| String::from_utf8_lossy(s))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default()
}

fn num_cpus() -> f64 {
    static NCPUS: OnceLock<f64> = OnceLock::new();
    *NCPUS.get_or_init(|| {
        std::fs::read_to_string("/proc/cpuinfo")
            .map(|c| c.lines().filter(|l| l.starts_with("processor")).count() as f64)
            .unwrap_or(1.0)
    })
}

fn resolve_uid(uid: u32) -> String {
    static USERS: OnceLock<HashMap<u32, String>> = OnceLock::new();
    let map = USERS.get_or_init(|| {
        let mut m = HashMap::new();
        if let Ok(content) = std::fs::read_to_string("/etc/passwd") {
            for line in content.lines() {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 3 {
                    if let Ok(id) = parts[2].parse::<u32>() {
                        m.insert(id, parts[0].to_string());
                    }
                }
            }
        }
        m
    });
    map.get(&uid).cloned().unwrap_or_else(|| uid.to_string())
}

// ─── Sorting and filtering helpers ────────────────────────────────────────────

/// Sorts a process list in-place by the given criterion.
pub fn sort(procs: &mut Vec<ProcessInfo>, sort_by: &str) {
    match sort_by {
        "cpu" => procs.sort_by(|a, b| {
            b.cpu_percent
                .partial_cmp(&a.cpu_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        "mem" => procs.sort_by_key(|p| std::cmp::Reverse(p.mem_rss_bytes)),
        "pid" => procs.sort_by_key(|p| p.pid),
        "name" => procs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
        "io" => procs.sort_by_key(|p| std::cmp::Reverse(p.io_read_bytes + p.io_write_bytes)),
        _ => {}
    }
}

/// Filters processes whose `name` or `cmdline` contains `filter` (case-insensitive).
pub fn filter_by_name<'a>(procs: &'a [ProcessInfo], filter: &str) -> Vec<ProcessInfo>
where
    ProcessInfo: Clone,
{
    if filter.is_empty() {
        return procs.to_vec();
    }
    let lower = filter.to_lowercase();
    procs
        .iter()
        .filter(|p| {
            p.name.to_lowercase().contains(&lower) || p.cmdline.to_lowercase().contains(&lower)
        })
        .cloned()
        .collect()
}
