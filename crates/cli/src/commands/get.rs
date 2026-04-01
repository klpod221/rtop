//! `rtop get` — collect and print system metrics to stdout.

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use clap::Args;

use app_collector::{cpu, disk, gpu, mem, net, proc, system, AgentPayload};

#[derive(Args)]
pub struct GetArgs {
    /// Modules to collect (comma-separated): host,cpu,mem,disk,net,proc,gpu,all
    #[arg(long, default_value = "all")]
    modules: String,

    /// Output file path (default: stdout)
    #[arg(long)]
    output: Option<std::path::PathBuf>,

    /// Output format: json | flat
    #[arg(long, default_value = "json")]
    format: String,

    /// Collection interval in milliseconds
    #[arg(long, default_value_t = 1000)]
    interval: u64,

    /// Number of cycles (0 = infinite)
    #[arg(long, default_value_t = 1)]
    count: u64,

    /// Compact JSON output (no indent)
    #[arg(long)]
    compact: bool,

    /// CPU fields to include (csv): usage,freq,temp,power,loadavg,uptime,name,battery
    #[arg(long, default_value = "")]
    cpu_fields: String,

    /// Sort processes by: cpu | mem | pid | name | io
    #[arg(long, default_value = "cpu")]
    proc_sort: String,

    /// Limit number of processes (0 = all)
    #[arg(long, default_value_t = 0)]
    proc_top: usize,

    /// Filter processes by name substring
    #[arg(long, default_value = "")]
    proc_filter: String,

    /// Skip process collection entirely
    #[arg(long)]
    no_proc: bool,

    /// Filter network interfaces (csv)
    #[arg(long, default_value = "")]
    net_iface: String,

    /// Filter disk mount points (csv)
    #[arg(long, default_value = "")]
    disk_mount: String,
}

pub async fn run(args: GetArgs, cfg_path: &Path) -> Result<()> {
    let _cfg = app_config::load(cfg_path).unwrap_or_default();

    let module_set = parse_modules(&args.modules, args.no_proc);

    // Warm-up GPU + CPU for delta-based collectors
    let mut intel_col: Option<gpu::intel::IntelGpuCollector> = None;
    if module_set.get("gpu").copied().unwrap_or(false) {
        if let Ok(col) = gpu::intel::IntelGpuCollector::new() {
            intel_col = Some(col);
            intel_col.as_mut().unwrap().collect();
        }
        #[cfg(feature = "nvidia")]
        gpu::nvidia::init();
    }
    if module_set.get("cpu").copied().unwrap_or(false) {
        cpu::collect().ok();
    }

    let net_ifaces: Vec<String> = split_csv(&args.net_iface);
    let disk_mounts: Vec<String> = split_csv(&args.disk_mount);
    let cpu_fields: Vec<String> = split_csv(&args.cpu_fields);

    let mut iteration = 0u64;
    loop {
        tokio::time::sleep(Duration::from_millis(args.interval)).await;
        iteration += 1;

        let payload = build_payload(
            &module_set, &args, &cpu_fields, &net_ifaces, &disk_mounts,
            intel_col.as_mut(),
        );

        let json_bytes = if args.compact {
            serde_json::to_vec(&payload)?
        } else {
            serde_json::to_vec_pretty(&payload)?
        };

        let output = if args.format == "flat" {
            flatten_json(&json_bytes)?
        } else {
            json_bytes
        };

        if let Some(ref path) = args.output {
            std::fs::write(path, &output)?;
            eprintln!("[{iteration}] wrote {} bytes to {}", output.len(), path.display());
        } else {
            println!("{}", String::from_utf8_lossy(&output));
        }

        if args.count > 0 && iteration >= args.count { break; }
    }
    Ok(())
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn parse_modules(modules: &str, no_proc: bool) -> HashMap<String, bool> {
    let mut set = HashMap::new();
    if modules == "all" {
        for m in ["host", "cpu", "mem", "disk", "net", "proc", "gpu"] {
            set.insert(m.to_string(), m != "proc" || !no_proc);
        }
    } else {
        for m in modules.split(',') {
            set.insert(m.trim().to_string(), true);
        }
    }
    if no_proc { set.insert("proc".into(), false); }
    set
}

fn split_csv(s: &str) -> Vec<String> {
    if s.is_empty() { return Vec::new(); }
    s.split(',').map(|v| v.trim().to_string()).collect()
}

fn build_payload(
    modules: &HashMap<String, bool>,
    args: &GetArgs,
    cpu_fields: &[String],
    net_ifaces: &[String],
    disk_mounts: &[String],
    intel_col: Option<&mut gpu::intel::IntelGpuCollector>,
) -> AgentPayload {
    use std::time::SystemTime;
    let mut payload = AgentPayload {
        timestamp: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0),
        ..Default::default()
    };

    if modules.get("host").copied().unwrap_or(false) {
        payload.host = Some(system::collect());
    }
    if modules.get("cpu").copied().unwrap_or(false) {
        if let Ok(mut stats) = cpu::collect() {
            if !cpu_fields.is_empty() {
                stats = cpu::filter_fields(stats, cpu_fields);
            }
            payload.cpu = Some(stats);
        }
    }
    if modules.get("mem").copied().unwrap_or(false) {
        payload.memory = mem::collect().ok();
    }
    if modules.get("disk").copied().unwrap_or(false) {
        payload.disks_space = disk::collect_space(disk_mounts);
        payload.disks_io    = disk::collect_io();
    }
    if modules.get("net").copied().unwrap_or(false) {
        payload.network = net::collect(net_ifaces, false).unwrap_or_default();
    }
    if modules.get("proc").copied().unwrap_or(false) {
        let total_mem = payload.memory.as_ref().map(|m| m.total)
            .unwrap_or_else(|| mem::collect().map(|m| m.total).unwrap_or(0));
        let mut procs = proc::collect(total_mem);
        if !args.proc_filter.is_empty() {
            procs = proc::filter_by_name(&procs, &args.proc_filter);
        }
        proc::sort(&mut procs, &args.proc_sort);
        if args.proc_top > 0 { procs.truncate(args.proc_top); }
        payload.processes = procs;
    }
    if modules.get("gpu").copied().unwrap_or(false) {
        if let Some(col) = intel_col {
            let stats = col.collect();
            if !stats.engines.is_empty() || stats.freq_act_mhz > 0.0 {
                payload.intel_gpu = Some(stats);
            }
        }
        #[cfg(feature = "nvidia")]
        {
            let nv = gpu::nvidia::collect();
            if !nv.is_empty() { payload.nvidia_gpus = nv; }
        }
        let amd = gpu::amd::collect();
        if !amd.is_empty() { payload.amd_gpus = amd; }
    }
    payload
}

fn flatten_json(data: &[u8]) -> Result<Vec<u8>> {
    let val: serde_json::Value = serde_json::from_slice(data)?;
    let mut flat = serde_json::Map::new();
    flatten_value("", &val, &mut flat);
    Ok(serde_json::to_vec_pretty(&flat)?)
}

fn flatten_value(prefix: &str, val: &serde_json::Value, dst: &mut serde_json::Map<String, serde_json::Value>) {
    match val {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                let key = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
                flatten_value(&key, v, dst);
            }
        }
        other => { dst.insert(prefix.to_string(), other.clone()); }
    }
}
