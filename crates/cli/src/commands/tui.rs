//! `rtop tui` — btop-style interactive terminal UI.
//!
//! Layout (4 panels):
//!   ┌───────────────────────────────────────────────────────────┐
//!   │ CPU — per-core sparklines, load avg, temp, freq, power   │
//!   ├──────────────────┬────────────────────────────────────────┤
//!   │ MEM/SWAP bars    │ NET/DISK/GPU stats                    │
//!   ├──────────────────┴────────────────────────────────────────┤
//!   │ PROCESS table (sortable, filterable, kill support)       │
//!   └───────────────────────────────────────────────────────────┘

use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers,
        MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Gauge, Paragraph, Row, Sparkline, Table,
        TableState,
    },
    Frame, Terminal,
};

use app_collector::{
    cpu::{self, CpuStats},
    disk::{self, DiskIo, DiskSpace},
    gpu::{self, AmdGpuStats, IntelGpuStats, NvidiaGpuStats},
    mem::{self, MemStats},
    net::{self, NetInterface},
    proc::{self, ProcessInfo},
};

// ─── Refresh rate ─────────────────────────────────────────────────────────────

const REFRESH_MS: u64 = 1000;
const SPARKLINE_LEN: usize = 60;

// ─── Theme ────────────────────────────────────────────────────────────────────

struct Theme {
    header_fg: Color,
    accent:    Color,
    good:      Color,
    warn:      Color,
    critical:  Color,
    text:      Color,
    dim:       Color,
    selected:  Color,
}

impl Theme {
    fn default_dark() -> Self {
        Self {
            header_fg: Color::Cyan,
            accent:    Color::Rgb(100, 180, 255),
            good:      Color::Rgb(80,  200, 120),
            warn:      Color::Rgb(255, 200, 50),
            critical:  Color::Rgb(255, 80,  80),
            text:      Color::Rgb(220, 220, 220),
            dim:       Color::Rgb(120, 120, 140),
            selected:  Color::Rgb(40,  60,  100),
        }
    }
}

// ─── UI state ─────────────────────────────────────────────────────────────────

/// Which panel currently has keyboard focus.
#[derive(Clone, Copy, PartialEq)]
enum Focus { Cpu, MemNet, Proc }

/// Sort column for the process table.
#[derive(Clone, Copy, PartialEq)]
enum ProcSort { Cpu, Mem, Pid, Name, Io }

struct AppState {
    // Metrics snapshots
    cpu: CpuStats,
    mem: MemStats,
    disks_space: Vec<DiskSpace>,
    disks_io:    Vec<DiskIo>,
    net:         Vec<NetInterface>,
    procs:       Vec<ProcessInfo>,
    intel_gpu:   Option<IntelGpuStats>,
    nvidia_gpus: Vec<NvidiaGpuStats>,
    amd_gpus:    Vec<AmdGpuStats>,

    // Sparkline history (per core, last SPARKLINE_LEN values)
    core_history: Vec<Vec<u64>>,
    cpu_history:  Vec<u64>,

    // Process table
    proc_table:  TableState,
    proc_sort:   ProcSort,
    proc_filter: String,
    filter_mode: bool,

    // Panel focus
    focus: Focus,
    // Panel expand (true = full screen)
    expanded: bool,

    theme: Theme,
}

impl AppState {
    fn new() -> Self {
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
            core_history: Vec::new(),
            cpu_history: Vec::new(),
            proc_table: TableState::default(),
            proc_sort: ProcSort::Cpu,
            proc_filter: String::new(),
            filter_mode: false,
            focus: Focus::Proc,
            expanded: false,
            theme: Theme::default_dark(),
        }
    }

    fn update_metrics(
        &mut self,
        intel_col: &mut Option<gpu::intel::IntelGpuCollector>,
    ) {
        self.cpu = cpu::collect().unwrap_or_default();
        self.mem = mem::collect().unwrap_or_default();
        self.disks_space = disk::collect_space(&[]);
        self.disks_io    = disk::collect_io();
        self.net         = net::collect(&[], false).unwrap_or_default();

        let total_mem = self.mem.total;
        let mut procs = proc::collect(total_mem);
        if !self.proc_filter.is_empty() {
            procs = proc::filter_by_name(&procs, &self.proc_filter);
        }
        self.sort_procs(&mut procs);
        self.procs = procs;

        if let Some(col) = intel_col {
            let s = col.collect();
            self.intel_gpu = if !s.engines.is_empty() || s.freq_act_mhz > 0.0 { Some(s) } else { None };
        }

        #[cfg(feature = "nvidia")]
        { self.nvidia_gpus = gpu::nvidia::collect(); }
        self.amd_gpus = gpu::amd::collect();

        // Update sparklines
        let overall = self.cpu.usage_percent.round() as u64;
        self.cpu_history.push(overall);
        if self.cpu_history.len() > SPARKLINE_LEN { self.cpu_history.remove(0); }

        let cores = &self.cpu.cores_percent;
        if self.core_history.len() != cores.len() {
            self.core_history = vec![vec![0u64; SPARKLINE_LEN]; cores.len()];
        }
        for (i, &pct) in cores.iter().enumerate() {
            self.core_history[i].push(pct.round() as u64);
            if self.core_history[i].len() > SPARKLINE_LEN { self.core_history[i].remove(0); }
        }
    }

    fn sort_procs(&self, procs: &mut Vec<ProcessInfo>) {
        let key = match self.proc_sort {
            ProcSort::Cpu  => "cpu",
            ProcSort::Mem  => "mem",
            ProcSort::Pid  => "pid",
            ProcSort::Name => "name",
            ProcSort::Io   => "io",
        };
        proc::sort(procs, key);
    }

    fn next_sort(&mut self) {
        self.proc_sort = match self.proc_sort {
            ProcSort::Cpu  => ProcSort::Mem,
            ProcSort::Mem  => ProcSort::Pid,
            ProcSort::Pid  => ProcSort::Name,
            ProcSort::Name => ProcSort::Io,
            ProcSort::Io   => ProcSort::Cpu,
        };
    }

    fn proc_scroll_down(&mut self) {
        if self.procs.is_empty() { return; }
        let i = self.proc_table.selected().map(|i| (i + 1).min(self.procs.len() - 1)).unwrap_or(0);
        self.proc_table.select(Some(i));
    }

    fn proc_scroll_up(&mut self) {
        let i = self.proc_table.selected().map(|i| i.saturating_sub(1)).unwrap_or(0);
        self.proc_table.select(Some(i));
    }

    fn kill_selected(&self) {
        if let Some(idx) = self.proc_table.selected() {
            if let Some(proc) = self.procs.get(idx) {
                // SAFETY: kill(2) is safe to call with a valid pid and SIGTERM
                unsafe { libc::kill(proc.pid as i32, libc::SIGTERM); }
            }
        }
    }
}

// ─── Entry point ──────────────────────────────────────────────────────────────

pub async fn run(cfg_path: &Path) -> Result<()> {
    let _cfg = app_config::load(cfg_path).unwrap_or_default();

    // Warm-up GPU + CPU
    let mut intel_col: Option<gpu::intel::IntelGpuCollector> =
        gpu::intel::IntelGpuCollector::new().ok();
    if let Some(ref mut c) = intel_col { c.collect(); }
    #[cfg(feature = "nvidia")]
    gpu::nvidia::init();
    cpu::collect().ok();

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = event_loop(&mut terminal, &mut intel_col).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    result
}

async fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    intel_col: &mut Option<gpu::intel::IntelGpuCollector>,
) -> Result<()> {
    let mut state = AppState::new();
    state.update_metrics(intel_col);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| draw(f, &mut state))?;

        let timeout = Duration::from_millis(REFRESH_MS)
            .checked_sub(last_tick.elapsed())
            .unwrap_or_default();

        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => {
                    if state.filter_mode {
                        match key.code {
                            KeyCode::Esc | KeyCode::Enter => { state.filter_mode = false; }
                            KeyCode::Backspace => { state.proc_filter.pop(); }
                            KeyCode::Char(c)   => { state.proc_filter.push(c); }
                            _ => {}
                        }
                    } else {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(()),
                            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return Ok(()),
                            KeyCode::Tab        => state.focus = match state.focus {
                                Focus::Cpu    => Focus::MemNet,
                                Focus::MemNet => Focus::Proc,
                                Focus::Proc   => Focus::Cpu,
                            },
                            KeyCode::Char('/')  => { state.filter_mode = true; state.proc_filter.clear(); }
                            KeyCode::Char('e')  => state.expanded = !state.expanded,
                            KeyCode::Char('k') | KeyCode::Delete => state.kill_selected(),
                            KeyCode::Down | KeyCode::Char('j') => state.proc_scroll_down(),
                            KeyCode::Up   | KeyCode::Char('i') => state.proc_scroll_up(),
                            KeyCode::F(6) | KeyCode::Char('s') => state.next_sort(),
                            KeyCode::Char('?') | KeyCode::Char('h') => {} // TODO: help overlay
                            _ => {}
                        }
                    }
                }
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollDown => state.proc_scroll_down(),
                    MouseEventKind::ScrollUp   => state.proc_scroll_up(),
                    _ => {}
                },
                Event::Resize(_, _) => {}
                _ => {}
            }
        }

        if last_tick.elapsed() >= Duration::from_millis(REFRESH_MS) {
            state.update_metrics(intel_col);
            last_tick = Instant::now();
        }
    }
}

// ─── Rendering ────────────────────────────────────────────────────────────────

fn draw(f: &mut Frame, state: &mut AppState) {
    let _t = &state.theme; // used in sub-functions via state ref
    let area = f.area();

    // Root layout: CPU | [MEM/NET] | PROC
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),       // CPU
            Constraint::Length(8),       // MEM + NET/DISK/GPU
            Constraint::Min(10),         // PROC
        ])
        .split(area);

    draw_cpu(f, state, rows[0]);
    draw_mem_net(f, state, rows[1]);
    draw_proc(f, state, rows[2]);
}

// ── CPU panel ─────────────────────────────────────────────────────────────────

fn draw_cpu(f: &mut Frame, state: &AppState, area: Rect) {
    let t = &state.theme;
    let cpu = &state.cpu;

    let block = Block::default()
        .title(Span::styled(" CPU ", Style::default().fg(t.header_fg).add_modifier(Modifier::BOLD)))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(t.accent));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Header line: name, usage, load avg, uptime, temp, power
    let uptime_h = (cpu.uptime_seconds / 3600.0) as u64;
    let uptime_m = ((cpu.uptime_seconds % 3600.0) / 60.0) as u64;
    let header = format!(
        " {}  Usage: {:.1}%  Load: {:.2} {:.2} {:.2}  Up: {}h{}m  Temp: {}°C  Power: {:.1}W",
        cpu.cpu_name,
        cpu.usage_percent,
        cpu.load_avg[0], cpu.load_avg[1], cpu.load_avg[2],
        uptime_h, uptime_m,
        cpu.package_temp_c,
        cpu.power_watts,
    );

    let usage_color = pct_color(t, cpu.usage_percent);
    let header_widget = Paragraph::new(header)
        .style(Style::default().fg(t.text));

    // Overall sparkline
    let sparkline = Sparkline::default()
        .data(&state.cpu_history)
        .max(100)
        .style(Style::default().fg(usage_color));

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(2), Constraint::Min(1)])
        .split(inner);

    f.render_widget(header_widget, rows[0]);
    f.render_widget(sparkline, rows[1]);

    // Per-core bars (compact gauge row)
    let cols_per_row = rows[2].width as usize / 12; // ~"C0 [██░] 72%" = 12 chars
    let cores = &cpu.cores_percent;
    let mut core_lines: Vec<Line> = Vec::new();
    let mut line_spans: Vec<Span> = Vec::new();
    for (i, &pct) in cores.iter().enumerate() {
        let color = pct_color(t, pct);
        let bar = build_mini_bar(pct, 6);
        line_spans.push(Span::styled(format!("C{i} "), Style::default().fg(t.dim)));
        line_spans.push(Span::styled(format!("[{bar}] "), Style::default().fg(color)));
        line_spans.push(Span::styled(format!("{pct:5.1}%  "), Style::default().fg(t.text)));
        if (i + 1) % cols_per_row.max(1) == 0 {
            core_lines.push(Line::from(std::mem::take(&mut line_spans)));
        }
    }
    if !line_spans.is_empty() { core_lines.push(Line::from(line_spans)); }
    f.render_widget(Paragraph::new(core_lines), rows[2]);
}

// ── MEM + NET/DISK/GPU panel ──────────────────────────────────────────────────

fn draw_mem_net(f: &mut Frame, state: &AppState, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);

    draw_mem(f, state, cols[0]);
    draw_net_disk_gpu(f, state, cols[1]);
}

fn draw_mem(f: &mut Frame, state: &AppState, area: Rect) {
    let t = &state.theme;
    let mem = &state.mem;

    let block = Block::default()
        .title(Span::styled(" MEM ", Style::default().fg(t.header_fg).bold()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(t.accent));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Length(1), Constraint::Length(2), Constraint::Min(0)])
        .split(inner);

    // RAM gauge
    let ram_pct = if mem.total > 0 { (mem.used as f64 / mem.total as f64 * 100.0) as u16 } else { 0 };
    let ram_label = format!("{} / {}", fmt_bytes(mem.used), fmt_bytes(mem.total));
    let ram_gauge = Gauge::default()
        .block(Block::default().title("RAM").borders(Borders::NONE))
        .gauge_style(Style::default().fg(pct_color(t, ram_pct as f64)).bg(Color::Rgb(30, 30, 40)))
        .percent(ram_pct)
        .label(ram_label);
    f.render_widget(ram_gauge, rows[0]);

    // Swap gauge
    let swap_pct = if mem.swap_total > 0 { (mem.swap_used as f64 / mem.swap_total as f64 * 100.0) as u16 } else { 0 };
    let swap_label = format!("{} / {}", fmt_bytes(mem.swap_used), fmt_bytes(mem.swap_total));
    let swap_gauge = Gauge::default()
        .block(Block::default().title("SWP").borders(Borders::NONE))
        .gauge_style(Style::default().fg(pct_color(t, swap_pct as f64)).bg(Color::Rgb(30, 30, 40)))
        .percent(swap_pct)
        .label(swap_label);
    f.render_widget(swap_gauge, rows[2]);
}

fn draw_net_disk_gpu(f: &mut Frame, state: &AppState, area: Rect) {
    let t = &state.theme;
    let block = Block::default()
        .title(Span::styled(" NET / DISK / GPU ", Style::default().fg(t.header_fg).bold()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(t.accent));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();

    // Network
    for iface in &state.net {
        if iface.name == "lo" { continue; }
        lines.push(Line::from(vec![
            Span::styled(format!(" {:8} ", iface.name), Style::default().fg(t.accent)),
            Span::styled("↑ ", Style::default().fg(t.good)),
            Span::styled(fmt_bytes_per_sec(iface.tx_bytes_per_sec), Style::default().fg(t.text)),
            Span::styled("  ↓ ", Style::default().fg(Color::Rgb(100, 160, 255))),
            Span::styled(fmt_bytes_per_sec(iface.rx_bytes_per_sec), Style::default().fg(t.text)),
        ]));
    }

    // Disk IO
    for dio in &state.disks_io {
        lines.push(Line::from(vec![
            Span::styled(format!(" {:8} ", dio.device), Style::default().fg(Color::Rgb(180, 140, 255))),
            Span::styled("R ", Style::default().fg(t.good)),
            Span::styled(fmt_bytes_per_sec(dio.read_bytes_per_sec), Style::default().fg(t.text)),
            Span::styled("  W ", Style::default().fg(t.warn)),
            Span::styled(fmt_bytes_per_sec(dio.write_bytes_per_sec), Style::default().fg(t.text)),
        ]));
    }

    // GPU
    if let Some(igpu) = &state.intel_gpu {
        let render_eng = igpu.engines.iter().find(|e| e.name.contains("render")).map(|e| e.busy_pct).unwrap_or(0.0);
        lines.push(Line::from(vec![
            Span::styled(" Intel GPU  ", Style::default().fg(t.accent)),
            Span::styled(format!("{render_eng:.0}% @ {:.0}MHz  {:.1}W", igpu.freq_act_mhz, igpu.power_gpu_watts), Style::default().fg(t.text)),
        ]));
    }
    for ngpu in &state.nvidia_gpus {
        lines.push(Line::from(vec![
            Span::styled(format!(" {:10} ", truncate(&ngpu.name, 10)), Style::default().fg(Color::Rgb(120, 220, 120))),
            Span::styled(format!("{:.0}%  {}MiB  {}°C  {:.1}W", ngpu.usage_percent, ngpu.mem_used >> 20, ngpu.temp_c, ngpu.power_watts), Style::default().fg(t.text)),
        ]));
    }
    for agpu in &state.amd_gpus {
        lines.push(Line::from(vec![
            Span::styled(format!(" {:10} ", truncate(&agpu.name, 10)), Style::default().fg(Color::Rgb(255, 80, 60))),
            Span::styled(format!("{:.0}%  {}MiB  {:.0}°C  {:.1}W", agpu.usage_percent, agpu.mem_used >> 20, agpu.temp_c, agpu.power_watts), Style::default().fg(t.text)),
        ]));
    }

    f.render_widget(Paragraph::new(lines), inner);
}

// ── Process table ─────────────────────────────────────────────────────────────

fn draw_proc(f: &mut Frame, state: &mut AppState, area: Rect) {
    let t = &state.theme;
    let sort_label = match state.proc_sort {
        ProcSort::Cpu  => "CPU▼",
        ProcSort::Mem  => "MEM▼",
        ProcSort::Pid  => "PID▼",
        ProcSort::Name => "NAME▼",
        ProcSort::Io   => "IO▼",
    };
    let filter_str = if state.filter_mode {
        format!(" Filter: {}█", state.proc_filter)
    } else if !state.proc_filter.is_empty() {
        format!(" Filter: {}", state.proc_filter)
    } else {
        String::new()
    };

    let title = format!(" PROCESSES ({sort_label}){filter_str}  [/]=filter [s]=sort [k]=kill [Tab]=focus [q]=quit ");
    let block = Block::default()
        .title(Span::styled(title, Style::default().fg(t.header_fg).bold()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(
            if state.focus == Focus::Proc { t.accent } else { t.dim }
        ));

    let header_cells = ["PID", "NAME", "CMD", "CPU%", "MEM%", "MEM RSS", "IO R/s", "IO W/s"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(t.header_fg).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1).bottom_margin(0);

    let rows: Vec<Row> = state.procs.iter().map(|p| {
        let cpu_color = pct_color(t, p.cpu_percent);
        Row::new(vec![
            Cell::from(p.pid.to_string()).style(Style::default().fg(t.dim)),
            Cell::from(truncate(&p.name, 16)).style(Style::default().fg(t.text).add_modifier(Modifier::BOLD)),
            Cell::from(truncate(&p.cmdline, 24)).style(Style::default().fg(t.dim)),
            Cell::from(format!("{:.1}", p.cpu_percent)).style(Style::default().fg(cpu_color)),
            Cell::from(format!("{:.1}", p.mem_percent)).style(Style::default().fg(pct_color(t, p.mem_percent))),
            Cell::from(fmt_bytes(p.mem_rss_bytes)).style(Style::default().fg(t.text)),
            Cell::from(fmt_bytes_per_sec(p.io_read_bytes as f64)).style(Style::default().fg(t.good)),
            Cell::from(fmt_bytes_per_sec(p.io_write_bytes as f64)).style(Style::default().fg(t.warn)),
        ])
    }).collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(7),   // PID
            Constraint::Length(17),  // NAME
            Constraint::Min(15),     // CMD
            Constraint::Length(6),   // CPU%
            Constraint::Length(6),   // MEM%
            Constraint::Length(9),   // MEM RSS
            Constraint::Length(9),   // IO R
            Constraint::Length(9),   // IO W
        ],
    )
    .header(header)
    .block(block)
    .row_highlight_style(Style::default().bg(t.selected))
    .highlight_symbol("▶ ");

    f.render_stateful_widget(table, area, &mut state.proc_table);
}

// ─── Utilities ────────────────────────────────────────────────────────────────

fn pct_color(t: &Theme, pct: f64) -> Color {
    if pct >= 90.0 { t.critical }
    else if pct >= 70.0 { t.warn }
    else { t.good }
}

fn build_mini_bar(pct: f64, width: usize) -> String {
    let filled = ((pct / 100.0) * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

fn fmt_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB"];
    let mut val = bytes as f64;
    let mut unit = 0;
    while val >= 1024.0 && unit + 1 < UNITS.len() { val /= 1024.0; unit += 1; }
    if unit == 0 { format!("{bytes}B") } else { format!("{val:.1}{}", UNITS[unit]) }
}

fn fmt_bytes_per_sec(bps: f64) -> String {
    const UNITS: &[&str] = &["B/s", "KB/s", "MB/s", "GB/s"];
    let mut val = bps;
    let mut unit = 0;
    while val >= 1024.0 && unit + 1 < UNITS.len() { val /= 1024.0; unit += 1; }
    format!("{val:6.1}{}", UNITS[unit])
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() } else { format!("{}…", &s[..max - 1]) }
}
