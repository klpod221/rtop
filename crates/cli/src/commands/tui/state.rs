use ratatui::widgets::TableState;
use std::collections::VecDeque;

use app_collector::{
    cpu::CpuStats,
    disk::{DiskIo, DiskSpace},
    gpu::{AmdGpuStats, IntelGpuStats, NvidiaGpuStats},
    mem::MemStats,
    net::NetInterface,
    proc::ProcessInfo,
};

use super::theme::Theme;

#[derive(Clone, Copy, PartialEq)]
pub enum Focus {
    CpuGraph,
    CpuDetail,
    Mem,
    Disk,
    Net,
    Proc,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ProcSort {
    Cpu,
    Mem,
    Pid,
    Name,
    Threads,
}

pub struct AppState {
    pub cpu: CpuStats,
    pub mem: MemStats,
    pub disks_space: Vec<DiskSpace>,
    pub disks_io: Vec<DiskIo>,
    pub net: Vec<NetInterface>,
    pub procs: Vec<ProcessInfo>,
    pub intel_gpu: Option<IntelGpuStats>,
    pub nvidia_gpus: Vec<NvidiaGpuStats>,
    pub amd_gpus: Vec<AmdGpuStats>,

    pub cpu_history: VecDeque<u64>,
    pub gpu_history: VecDeque<u64>,
    pub core_history: Vec<VecDeque<u64>>,
    pub net_rx_history: VecDeque<f64>,
    pub net_tx_history: VecDeque<f64>,
    pub net_rx_peak: f64,
    pub net_tx_peak: f64,

    pub focus: Focus,
    pub proc_table: TableState,
    pub proc_sort: ProcSort,
    pub proc_filter: String,
    pub filter_mode: bool,
    pub reverse_sort: bool,
    pub tree_mode: bool,
    pub per_core_mode: bool,
    pub selected_iface: usize,
    pub theme: Theme,
    pub refresh_ms: u64,
}

impl AppState {
    pub const HISTORY_LEN: usize = 200;

    pub fn new(refresh_ms: u64) -> Self {
        Self {
            cpu: CpuStats::default(),
            mem: MemStats::default(),
            disks_space: Vec::new(),
            disks_io: Vec::new(),
            net: Vec::new(),
            procs: Vec::new(),
            intel_gpu: None,
            nvidia_gpus: Vec::new(),
            amd_gpus: Vec::new(),

            cpu_history: VecDeque::with_capacity(Self::HISTORY_LEN),
            gpu_history: VecDeque::with_capacity(Self::HISTORY_LEN),
            core_history: Vec::new(),
            net_rx_history: VecDeque::with_capacity(Self::HISTORY_LEN),
            net_tx_history: VecDeque::with_capacity(Self::HISTORY_LEN),
            net_rx_peak: 0.0,
            net_tx_peak: 0.0,

            focus: Focus::Proc,
            proc_table: TableState::default(),
            proc_sort: ProcSort::Cpu,
            proc_filter: String::new(),
            filter_mode: false,
            reverse_sort: false,
            tree_mode: false,
            per_core_mode: false,
            selected_iface: 0,
            theme: Theme::default_dark(),
            refresh_ms,
        }
    }

    pub fn next_focus(&mut self) {
        self.focus = match self.focus {
            Focus::CpuGraph => Focus::CpuDetail,
            Focus::CpuDetail => Focus::Mem,
            Focus::Mem => Focus::Disk,
            Focus::Disk => Focus::Net,
            Focus::Net => Focus::Proc,
            Focus::Proc => Focus::CpuGraph,
        };
    }
}
