//! rtop-collector — Linux system metrics library.
//!
//! # Usage
//! ```rust,no_run
//! use app_collector::{cpu, mem, disk, net, proc, system, gpu};
//!
//! let cpu_stats = cpu::collect().unwrap();
//! let mem_stats = mem::collect().unwrap();
//! ```

pub mod cpu;
pub mod disk;
pub mod gpu;
pub mod mem;
pub mod net;
pub mod payload;
pub mod proc;
pub mod system;

pub use payload::AgentPayload;

use std::time::SystemTime;
use app_config::Config;

/// Collects all enabled metrics according to the provided config and returns a
/// populated `AgentPayload`. This is the single entry-point used by both the
/// agent daemon and the web server.
pub fn collect_all(
    cfg: &Config,
    intel_col: Option<&mut gpu::intel::IntelGpuCollector>,
    machine_id: Option<&str>,
    machine_name: Option<&str>,
) -> AgentPayload {
    let m = &cfg.modules;
    let mut payload = AgentPayload {
        timestamp: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0),
        machine_id: machine_id.map(str::to_string),
        machine_name: machine_name.map(str::to_string),
        tags: cfg.agent.tags.clone(),
        ..Default::default()
    };

    if m.host.enabled {
        payload.host = Some(system::collect());
    }

    if m.cpu.enabled {
        if let Ok(mut stats) = cpu::collect() {
            if !m.cpu.fields.is_empty() {
                stats = cpu::filter_fields(stats, &m.cpu.fields);
            }
            payload.cpu = Some(stats);
        }
    }

    if m.memory.enabled {
        payload.memory = mem::collect().ok();
    }

    if m.disk.enabled {
        payload.disks_space = disk::collect_space(&m.disk.mount_filter);
        payload.disks_io    = disk::collect_io();
    }

    if m.network.enabled {
        payload.network = net::collect(&m.network.iface_filter, m.network.exclude_virtual)
            .unwrap_or_default();
    }

    if m.processes.enabled {
        let total_mem = payload.memory.as_ref().map(|m| m.total).unwrap_or_else(|| {
            mem::collect().map(|m| m.total).unwrap_or(0)
        });
        let mut procs = proc::collect(total_mem);
        if !m.processes.name_filter.is_empty() {
            procs = proc::filter_by_name(&procs, &m.processes.name_filter);
        }
        proc::sort(&mut procs, &m.processes.sort_by);
        if m.processes.top_n > 0 {
            procs.truncate(m.processes.top_n);
        }
        payload.processes = procs;
    }

    if m.gpu.enabled {
        // Intel
        if m.gpu.intel {
            if let Some(col) = intel_col {
                let stats = col.collect();
                if !stats.engines.is_empty() || stats.freq_act_mhz > 0.0 {
                    payload.intel_gpu = Some(stats);
                }
            }
        }
        // NVIDIA
        #[cfg(feature = "nvidia")]
        if m.gpu.nvidia {
            let nv = gpu::nvidia::collect();
            if !nv.is_empty() { payload.nvidia_gpus = nv; }
        }
        // AMD
        if m.gpu.amd {
            let amd = gpu::amd::collect();
            if !amd.is_empty() { payload.amd_gpus = amd; }
        }
    }

    payload
}
