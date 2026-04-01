//! Shared payload types transmitted by the agent and served by the web/MCP servers.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    cpu::CpuStats,
    disk::{DiskIo, DiskSpace},
    gpu::{AmdGpuStats, IntelGpuStats, NvidiaGpuStats},
    mem::MemStats,
    net::NetInterface,
    proc::ProcessInfo,
    system::HostInfo,
};

/// Full telemetry snapshot emitted every collection cycle.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentPayload {
    /// Unix timestamp in milliseconds.
    pub timestamp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub machine_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub machine_name: Option<String>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub tags: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<HostInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu: Option<CpuStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<MemStats>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub disks_space: Vec<DiskSpace>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub disks_io: Vec<DiskIo>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub network: Vec<NetInterface>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub processes: Vec<ProcessInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intel_gpu: Option<IntelGpuStats>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub nvidia_gpus: Vec<NvidiaGpuStats>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub amd_gpus: Vec<AmdGpuStats>,
}
