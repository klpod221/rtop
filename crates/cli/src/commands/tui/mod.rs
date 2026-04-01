pub mod layout;
pub mod panels;
pub mod state;
pub mod theme;
pub mod utils;

use std::collections::VecDeque;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal, Frame};

use app_collector::{cpu, disk, gpu, mem, net, proc};
use state::{AppState, ProcSort};

pub async fn run(_cfg_path: &Path) -> Result<()> {
    let refresh_ms = 1000; // default, would be read from app_config
    
    // Warm-up collectors
    let mut intel_col = gpu::intel::IntelGpuCollector::new().ok();
    if let Some(ref mut c) = intel_col {
        c.collect();
    }
    #[cfg(feature = "nvidia")]
    gpu::nvidia::init();
    cpu::collect().ok();

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = event_loop(&mut terminal, &mut intel_col, refresh_ms).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

async fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    intel_col: &mut Option<gpu::intel::IntelGpuCollector>,
    refresh_ms: u64,
) -> Result<()> {
    let mut state = AppState::new(refresh_ms);
    update_metrics(&mut state, intel_col);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| draw_all(f, &mut state))?;

        let timeout = Duration::from_millis(refresh_ms)
            .checked_sub(last_tick.elapsed())
            .unwrap_or_default();

        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => {
                    if state.filter_mode {
                        match key.code {
                            KeyCode::Esc | KeyCode::Enter => state.filter_mode = false,
                            KeyCode::Backspace => { state.proc_filter.pop(); }
                            KeyCode::Char(c) => state.proc_filter.push(c),
                            _ => {}
                        }
                    } else {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(()),
                            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return Ok(()),
                            KeyCode::Tab => state.next_focus(),
                            KeyCode::Char('/') => {
                                state.filter_mode = true;
                                state.proc_filter.clear();
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                let i = state.proc_table.selected().unwrap_or(0);
                                state.proc_table.select(Some(i.saturating_add(1).min(state.procs.len().saturating_sub(1))));
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                let i = state.proc_table.selected().unwrap_or(0);
                                state.proc_table.select(Some(i.saturating_sub(1)));
                            }
                            KeyCode::F(6) | KeyCode::Char('s') => {
                                state.proc_sort = match state.proc_sort {
                                    ProcSort::Cpu => ProcSort::Mem,
                                    ProcSort::Mem => ProcSort::Pid,
                                    ProcSort::Pid => ProcSort::Name,
                                    ProcSort::Name => ProcSort::Threads,
                                    ProcSort::Threads => ProcSort::Cpu,
                                };
                            }
                            KeyCode::Char('r') => state.reverse_sort = !state.reverse_sort,
                            KeyCode::Char('t') => state.tree_mode = !state.tree_mode,
                            KeyCode::Char('c') => state.per_core_mode = !state.per_core_mode,
                            KeyCode::Delete => {
                                if let Some(idx) = state.proc_table.selected() {
                                    if let Some(p) = state.procs.get(idx) {
                                        unsafe { libc::kill(p.pid as i32, libc::SIGTERM); }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Event::Mouse(mouse) => {
                    if mouse.kind == MouseEventKind::ScrollDown {
                        let i = state.proc_table.selected().unwrap_or(0);
                        state.proc_table.select(Some(i.saturating_add(1).min(state.procs.len().saturating_sub(1))));
                    } else if mouse.kind == MouseEventKind::ScrollUp {
                        let i = state.proc_table.selected().unwrap_or(0);
                        state.proc_table.select(Some(i.saturating_sub(1)));
                    } else if mouse.kind == MouseEventKind::Down(crossterm::event::MouseButton::Left) {
                        let _col = mouse.column;
                        let _row = mouse.row;
                        // TODO: map (col, row) to panel focus
                    }
                }
                _ => {}
            }
        }

        if last_tick.elapsed() >= Duration::from_millis(refresh_ms) {
            update_metrics(&mut state, intel_col);
            last_tick = Instant::now();
        }
    }
}

fn update_metrics(state: &mut AppState, intel_col: &mut Option<gpu::intel::IntelGpuCollector>) {
    state.cpu = cpu::collect().unwrap_or_default();
    state.mem = mem::collect().unwrap_or_default();
    state.disks_space = disk::collect_space(&[]);
    state.disks_io = disk::collect_io();
    state.net = net::collect(&[], false).unwrap_or_default();

    let total_mem = state.mem.total;
    let mut procs = proc::collect(total_mem);
    if !state.proc_filter.is_empty() {
        procs = proc::filter_by_name(&procs, &state.proc_filter);
    }
    
    // sorting
    proc::sort(&mut procs, match state.proc_sort {
        ProcSort::Cpu => "cpu",
        ProcSort::Mem => "mem",
        ProcSort::Pid => "pid",
        ProcSort::Name => "name",
        ProcSort::Threads => "threads", // Needs handle in proc::collect if threads supported
    });
    if state.reverse_sort {
        procs.reverse();
    }
    state.procs = procs;

    if let Some(col) = intel_col {
        let s = col.collect();
        state.intel_gpu = if !s.engines.is_empty() || s.freq_act_mhz > 0.0 { Some(s) } else { None };
    }
    #[cfg(feature = "nvidia")]
    { state.nvidia_gpus = gpu::nvidia::collect(); }
    state.amd_gpus = gpu::amd::collect();

    state.cpu_history.push_back(state.cpu.usage_percent.round() as u64);
    if state.cpu_history.len() > AppState::HISTORY_LEN { state.cpu_history.pop_front(); }

    // history max logic
    let mut max_gpu = 0.0_f64;
    if let Some(igpu) = &state.intel_gpu {
        max_gpu = max_gpu.max(igpu.engines.iter().find(|e| e.name.contains("render")).map(|e| e.busy_pct).unwrap_or(0.0));
    }
    for ngpu in &state.nvidia_gpus { max_gpu = max_gpu.max(ngpu.usage_percent); }
    for agpu in &state.amd_gpus { max_gpu = max_gpu.max(agpu.usage_percent); }
    
    state.gpu_history.push_back(max_gpu.round() as u64);
    if state.gpu_history.len() > AppState::HISTORY_LEN { state.gpu_history.pop_front(); }

    if state.core_history.len() != state.cpu.cores_percent.len() {
        state.core_history = vec![VecDeque::with_capacity(AppState::HISTORY_LEN); state.cpu.cores_percent.len()];
    }
    for (i, &pct) in state.cpu.cores_percent.iter().enumerate() {
        state.core_history[i].push_back(pct.round() as u64);
        if state.core_history[i].len() > AppState::HISTORY_LEN { state.core_history[i].pop_front(); }
    }
    
    // Net
    if let Some(n) = state.net.get(state.selected_iface) {
        state.net_rx_history.push_back(n.rx_bytes_per_sec);
        if state.net_rx_history.len() > AppState::HISTORY_LEN { state.net_rx_history.pop_front(); }
        state.net_rx_peak = state.net_rx_peak.max(n.rx_bytes_per_sec);
        
        state.net_tx_history.push_back(n.tx_bytes_per_sec);
        if state.net_tx_history.len() > AppState::HISTORY_LEN { state.net_tx_history.pop_front(); }
        state.net_tx_peak = state.net_tx_peak.max(n.tx_bytes_per_sec);
    }
}

fn draw_all(f: &mut Frame, state: &mut AppState) {
    let l = layout::build_layout(f.area(), state);
    panels::menu_bar::draw(f, state, l.menu_bar);
    panels::cpu_graph::draw(f, state, l.cpu_graph);
    panels::gpu_graph::draw(f, state, l.gpu_graph);
    panels::cpu_detail::draw(f, state, l.cpu_detail);
    panels::gpu_detail::draw(f, state, l.gpu_detail);
    panels::mem::draw(f, state, l.mem);
    panels::disk::draw(f, state, l.disks);
    panels::net::draw(f, state, l.net);
    panels::proc::draw(f, state, l.proc);
}
