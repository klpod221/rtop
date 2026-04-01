//! Configuration types, loader, and validator for rtop.
//!
//! Config file location: `$HOME/.config/rtop/config.json`
//! On first run, a default config is written automatically.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ─── Error type ────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("cannot determine home directory: {0}")]
    HomeDir(String),
    #[error("cannot read config file {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid JSON in config {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("config validation failed: {0}")]
    Validation(String),
    #[error("cannot write config {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: anyhow::Error,
    },
}

// ─── Structs ───────────────────────────────────────────────────────────────────

/// HTTP transport settings for the remote telemetry endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Full URL to POST telemetry to.
    pub endpoint: String,
    /// Value set in the auth header (empty = skip auth).
    pub auth_token: String,
    /// HTTP header name for the token. Default: `"Authorization"`.
    pub auth_header: String,
    /// HTTP request timeout in seconds.
    pub timeout_seconds: u64,
    /// Number of retries on failure.
    pub retry_count: u32,
    /// Base delay before first retry (exponential backoff).
    pub retry_delay_seconds: u64,
    /// Disable TLS certificate verification (testing only).
    pub tls_skip_verify: bool,
    /// Path to a custom PEM-encoded CA certificate file.
    pub tls_ca_cert: String,
    /// Enable gzip compression of the request body.
    pub compress: bool,
}

/// Runtime behavior of the agent daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentBehaviorConfig {
    /// Collection and send cycle in seconds.
    pub interval_seconds: u64,
    /// Stable unique ID for this machine (`/etc/machine-id` if empty).
    pub machine_id: String,
    /// Human-friendly name (hostname if empty).
    pub machine_name: String,
    /// Arbitrary key-value tags appended to every payload.
    pub tags: HashMap<String, String>,
    /// Verbosity: `"debug"`, `"info"`, `"warn"`, `"error"`.
    pub log_level: String,
    /// Log file path (stderr if empty).
    pub log_file: String,
    /// PID file path written while agent runs.
    pub pid_file: String,
}

/// Settings for the embedded Web UI server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebConfig {
    /// HTTP port the server listens on.
    pub port: u16,
    /// Preferred network interface to show in the topbar.
    pub network_interface: String,
    /// Restrict which mount points appear in the UI (empty = all).
    pub storage_filter: Vec<String>,
}

/// CPU module collection options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuModuleConfig {
    pub enabled: bool,
    /// Fields to include: `usage`, `freq`, `temp`, `power`, `loadavg`, `uptime`, `name`, `battery`.
    /// Empty list = all fields.
    pub fields: Vec<String>,
}

/// Disk module collection options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskModuleConfig {
    pub enabled: bool,
    /// Restrict to listed mount points (empty = all).
    pub mount_filter: Vec<String>,
}

/// Network module collection options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkModuleConfig {
    pub enabled: bool,
    /// Restrict to listed interface names (empty = all).
    pub iface_filter: Vec<String>,
    /// Filter out Docker bridges, veth pairs, and loopback.
    pub exclude_virtual: bool,
}

/// Process list collection options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessModuleConfig {
    pub enabled: bool,
    /// Limit to top N processes (0 = unlimited).
    pub top_n: usize,
    /// Sort order: `"cpu"`, `"mem"`, `"pid"`, `"name"`, `"io"`.
    pub sort_by: String,
    /// Retain only processes whose name/cmdline contains this substring.
    pub name_filter: String,
}

/// GPU collection options per vendor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuModuleConfig {
    pub enabled: bool,
    /// Intel GPU via perf PMU.
    pub intel: bool,
    /// NVIDIA via NVML (dynamic load).
    pub nvidia: bool,
    /// AMD via sysfs.
    pub amd: bool,
}

/// Toggle-only module config for modules with no extra settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleModuleConfig {
    pub enabled: bool,
}

/// Per-module enable/filter settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModulesConfig {
    pub host: SimpleModuleConfig,
    pub cpu: CpuModuleConfig,
    pub memory: SimpleModuleConfig,
    pub disk: DiskModuleConfig,
    pub network: NetworkModuleConfig,
    pub processes: ProcessModuleConfig,
    pub gpu: GpuModuleConfig,
}

/// Root rtop configuration.
/// Loaded from `$HOME/.config/rtop/config.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Enable the telemetry agent daemon.
    pub enabled_agent: bool,
    /// Enable the embedded Web UI server.
    pub enabled_web: bool,
    /// Data refresh interval in milliseconds (min 100). Shared by Web, TUI, and Agent.
    #[serde(default = "Config::default_interval_ms")]
    pub update_interval_ms: u64,
    pub web: WebConfig,
    pub server: ServerConfig,
    pub agent: AgentBehaviorConfig,
    pub modules: ModulesConfig,
}

// ─── Defaults ─────────────────────────────────────────────────────────────────

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://your-server:8080/api/telemetry".into(),
            auth_token: String::new(),
            auth_header: "Authorization".into(),
            timeout_seconds: 10,
            retry_count: 3,
            retry_delay_seconds: 5,
            tls_skip_verify: false,
            tls_ca_cert: String::new(),
            compress: false,
        }
    }
}

impl Default for AgentBehaviorConfig {
    fn default() -> Self {
        Self {
            interval_seconds: 5,
            machine_id: String::new(),
            machine_name: String::new(),
            tags: HashMap::new(),
            log_level: "info".into(),
            log_file: String::new(),
            pid_file: "/tmp/rtop-agent.pid".into(),
        }
    }
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            network_interface: String::new(),
            storage_filter: Vec::new(),
        }
    }
}

impl Default for ModulesConfig {
    fn default() -> Self {
        Self {
            host: SimpleModuleConfig { enabled: true },
            memory: SimpleModuleConfig { enabled: true },
            cpu: CpuModuleConfig {
                enabled: true,
                fields: vec![
                    "usage".into(),
                    "freq".into(),
                    "temp".into(),
                    "power".into(),
                    "loadavg".into(),
                    "uptime".into(),
                    "name".into(),
                ],
            },
            disk: DiskModuleConfig {
                enabled: true,
                mount_filter: Vec::new(),
            },
            network: NetworkModuleConfig {
                enabled: true,
                iface_filter: Vec::new(),
                exclude_virtual: true,
            },
            processes: ProcessModuleConfig {
                enabled: true,
                top_n: 20,
                sort_by: "cpu".into(),
                name_filter: String::new(),
            },
            gpu: GpuModuleConfig {
                enabled: true,
                intel: true,
                nvidia: true,
                amd: true,
            },
        }
    }
}

impl Config {
    pub const fn default_interval_ms() -> u64 { 1000 }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled_agent: false,
            enabled_web: false,
            update_interval_ms: 1000,
            web: WebConfig::default(),
            server: ServerConfig::default(),
            agent: AgentBehaviorConfig::default(),
            modules: ModulesConfig::default(),
        }
    }
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Returns the default config file path: `$HOME/.config/rtop/config.json`.
/// If running via `sudo`, it attempts to resolve the config of the `SUDO_USER`.
pub fn default_config_path() -> Result<PathBuf, ConfigError> {
    let home = std::env::var("SUDO_USER")
        .ok()
        .filter(|u| !u.is_empty() && u != "root")
        .and_then(|u| {
            std::fs::read_to_string("/etc/passwd")
                .unwrap_or_default()
                .lines()
                .find(|l| l.starts_with(&format!("{u}:")))
                .and_then(|l| l.split(':').nth(5).map(String::from))
        })
        .or_else(|| std::env::var("HOME").ok())
        .or_else(|| std::env::var("XDG_CONFIG_HOME").ok())
        .ok_or_else(|| ConfigError::HomeDir("HOME not set".into()))?;

    Ok(PathBuf::from(home).join(".config/rtop/config.json"))
}

/// Loads config from `path`. Returns default config if the file doesn't exist.
pub fn load(path: &Path) -> Result<Config, ConfigError> {
    if !path.exists() {
        return Ok(Config::default());
    }

    let data = std::fs::read(path).map_err(|e| ConfigError::Read {
        path: path.to_path_buf(),
        source: e,
    })?;

    // Deserialize over a default so unknown fields are ignored and missing
    // fields keep their defaults (same behaviour as Go's json.Unmarshal into
    // a pre-populated struct).
    let mut cfg = Config::default();
    let patch: serde_json::Value = serde_json::from_slice(&data).map_err(|e| ConfigError::Parse {
        path: path.to_path_buf(),
        source: e,
    })?;
    merge_json(&mut cfg, patch);

    validate(&cfg)?;
    Ok(cfg)
}

/// Serialises `cfg` to pretty JSON and writes it to `path`.
/// Parent directories are created automatically.
pub fn write(path: &Path, cfg: &Config) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating config directory {}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(cfg).context("serialising config")?;
    std::fs::write(path, json)
        .with_context(|| format!("writing config to {}", path.display()))?;
    Ok(())
}

/// Validates required fields and sanity-checks values.
pub fn validate(cfg: &Config) -> Result<(), ConfigError> {
    if cfg.agent.interval_seconds < 1 {
        return Err(ConfigError::Validation(
            "agent.interval_seconds must be >= 1".into(),
        ));
    }
    if cfg.server.timeout_seconds < 1 {
        return Err(ConfigError::Validation(
            "server.timeout_seconds must be >= 1".into(),
        ));
    }
    let valid_log_levels = ["debug", "info", "warn", "error"];
    if !valid_log_levels.contains(&cfg.agent.log_level.as_str()) {
        return Err(ConfigError::Validation(
            "agent.log_level must be one of: debug, info, warn, error".into(),
        ));
    }
    let valid_sort = ["cpu", "mem", "pid", "name", "io", ""];
    if !valid_sort.contains(&cfg.modules.processes.sort_by.as_str()) {
        return Err(ConfigError::Validation(
            "modules.processes.sort_by must be one of: cpu, mem, pid, name, io".into(),
        ));
    }
    if cfg.web.port == 0 {
        return Err(ConfigError::Validation("web.port must be > 0".into()));
    }
    if cfg.update_interval_ms < 100 {
        return Err(ConfigError::Validation("update_interval_ms must be >= 100".into()));
    }
    Ok(())
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Merges a partial JSON `patch` into `cfg`, preserving defaults for missing keys.
/// This mirrors Go's `json.Unmarshal` into a pre-populated struct.
fn merge_json(cfg: &mut Config, patch: serde_json::Value) {
    // Simplest correct approach: re-serialize cfg to Value, deep-merge, re-deserialize.
    let base = serde_json::to_value(&*cfg).unwrap_or(serde_json::Value::Null);
    let merged = deep_merge(base, patch);
    if let Ok(new_cfg) = serde_json::from_value::<Config>(merged) {
        *cfg = new_cfg;
    }
}

fn deep_merge(base: serde_json::Value, patch: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match (base, patch) {
        (Value::Object(mut base_map), Value::Object(patch_map)) => {
            for (k, v) in patch_map {
                let base_v = base_map.remove(&k).unwrap_or(Value::Null);
                base_map.insert(k, deep_merge(base_v, v));
            }
            Value::Object(base_map)
        }
        // Scalar / array: patch wins
        (_, patch) => patch,
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_validates() {
        let cfg = Config::default();
        assert!(validate(&cfg).is_ok());
    }

    #[test]
    fn invalid_log_level_rejected() {
        let mut cfg = Config::default();
        cfg.agent.log_level = "verbose".into();
        assert!(validate(&cfg).is_err());
    }

    #[test]
    fn roundtrip_json() {
        let cfg = Config::default();
        let json = serde_json::to_string_pretty(&cfg).unwrap();
        let loaded: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.web.port, cfg.web.port);
    }

    #[test]
    fn load_missing_file_returns_default() {
        let tmp = std::env::temp_dir().join(format!("app_config_test_{}.json", std::process::id()));
        let cfg = load(&tmp).unwrap();
        // File must NOT be created — only return defaults in memory
        assert!(!tmp.exists());
        assert_eq!(cfg.agent.interval_seconds, 5);
    }
}
