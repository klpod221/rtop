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
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Gauge, Paragraph, Row, Sparkline, Table, TableState},
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
    accent: Color,
    good: Color,
    warn: Color,
    critical: Color,
    text: Color,
    dim: Color,
    selected: Color,
}

impl Theme {
    fn default_dark() -> Self {
        Self {
            header_fg: Color::Cyan,
            accent: Color::Rgb(100, 180, 255),
            good: Color::Rgb(80, 200, 120),
            warn: Color::Rgb(255, 200, 50),
            critical: Color::Rgb(255, 80, 80),
            text: Color::Rgb(220, 220, 220),
            dim: Color::Rgb(120, 120, 140),
            selected: Color::Rgb(40, 60, 100),
        }
    }
}

// ─── UI state ─────────────────────────────────────────────────────────────────

/// Which panel currently has keyboard focus.
#[derive(Clone, Copy, PartialEq)]
enum Focus {
    Cpu,
    MemNet,
    Proc,
}

/// Sort column for the process table.
#[derive(Clone, Copy, PartialEq)]
enum ProcSort {
    Cpu,
    Mem,
    Pid,
    Name,
    Io,
}

struct AppState {
    // Metrics snapshots
    cpu: CpuStats,
    mem: MemStats,
    disks_space: Vec<DiskSpace>,
    disks_io: Vec<DiskIo>,
    net: Vec<NetInterface>,
    procs: Vec<ProcessInfo>,
    intel_gpu: Option<IntelGpuStats>,
    nvidia_gpus: Vec<NvidiaGpuStats>,
    amd_gpus: Vec<AmdGpuStats>,

    // Sparkline history (per core, last SPARKLINE_LEN values)
    core_history: Vec<Vec<u64>>,
    cpu_history: Vec<u64>,
    gpu_history: Vec<u64>,

    // Process table
    proc_table: TableState,
    proc_sort: ProcSort,
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
            gpu_history: Vec::new(),
            proc_table: TableState::default(),
            proc_sort: ProcSort::Cpu,
            proc_filter: String::new(),
            filter_mode: false,
            focus: Focus::Proc,
            expanded: false,
            theme: Theme::default_dark(),
        }
    }

    fn update_metrics(&mut self, intel_col: &mut Option<gpu::intel::IntelGpuCollector>) {
        self.cpu = cpu::collect().unwrap_or_default();
        self.mem = mem::collect().unwrap_or_default();
        self.disks_space = disk::collect_space(&[]);
        self.disks_io = disk::collect_io();
        self.net = net::collect(&[], false).unwrap_or_default();

        let total_mem = self.mem.total;
        let mut procs = proc::collect(total_mem);
        if !self.proc_filter.is_empty() {
            procs = proc::filter_by_name(&procs, &self.proc_filter);
        }
        self.sort_procs(&mut procs);
        self.procs = procs;

        if let Some(col) = intel_col {
            let s = col.collect();
            self.intel_gpu = if !s.engines.is_empty() || s.freq_act_mhz > 0.0 {
                Some(s)
            } else {
                None
            };
        }

        #[cfg(feature = "nvidia")]
        {
            self.nvidia_gpus = gpu::nvidia::collect();
        }
        self.amd_gpus = gpu::amd::collect();

        // Update sparklines
        let overall = self.cpu.usage_percent.round() as u64;
        self.cpu_history.push(overall);
        if self.cpu_history.len() > SPARKLINE_LEN {
            self.cpu_history.remove(0);
        }

        let mut max_gpu = 0.0_f64;
        if let Some(igpu) = &self.intel_gpu {
            max_gpu = max_gpu.max(
                igpu.engines
                    .iter()
                    .find(|e| e.name.contains("render"))
                    .map(|e| e.busy_pct)
                    .unwrap_or(0.0),
            );
        }
        for ngpu in &self.nvidia_gpus {
            max_gpu = max_gpu.max(ngpu.usage_percent);
        }
        for agpu in &self.amd_gpus {
            max_gpu = max_gpu.max(agpu.usage_percent);
        }
        self.gpu_history.push(max_gpu.round() as u64);
        if self.gpu_history.len() > SPARKLINE_LEN {
            self.gpu_history.remove(0);
        }

        let cores = &self.cpu.cores_percent;
        if self.core_history.len() != cores.len() {
            self.core_history = vec![vec![0u64; SPARKLINE_LEN]; cores.len()];
        }
        for (i, &pct) in cores.iter().enumerate() {
            self.core_history[i].push(pct.round() as u64);
            if self.core_history[i].len() > SPARKLINE_LEN {
                self.core_history[i].remove(0);
            }
        }
    }

    fn sort_procs(&self, procs: &mut Vec<ProcessInfo>) {
        let key = match self.proc_sort {
            ProcSort::Cpu => "cpu",
            ProcSort::Mem => "mem",
            ProcSort::Pid => "pid",
            ProcSort::Name => "name",
            ProcSort::Io => "io",
        };
        proc::sort(procs, key);
    }

    fn next_sort(&mut self) {
        self.proc_sort = match self.proc_sort {
            ProcSort::Cpu => ProcSort::Mem,
            ProcSort::Mem => ProcSort::Pid,
            ProcSort::Pid => ProcSort::Name,
            ProcSort::Name => ProcSort::Io,
            ProcSort::Io => ProcSort::Cpu,
        };
    }

    fn proc_scroll_down(&mut self) {
        if self.procs.is_empty() {
            return;
        }
        let i = self
            .proc_table
            .selected()
            .map(|i| (i + 1).min(self.procs.len() - 1))
            .unwrap_or(0);
        self.proc_table.select(Some(i));
    }

    fn proc_scroll_up(&mut self) {
        let i = self
            .proc_table
            .selected()
            .map(|i| i.saturating_sub(1))
            .unwrap_or(0);
        self.proc_table.select(Some(i));
    }

    fn kill_selected(&self) {
        if let Some(idx) = self.proc_table.selected() {
            if let Some(proc) = self.procs.get(idx) {
                // SAFETY: kill(2) is safe to call with a valid pid and SIGTERM
                unsafe {
                    libc::kill(proc.pid as i32, libc::SIGTERM);
                }
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
    if let Some(ref mut c) = intel_col {
        c.collect();
    }
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
                            KeyCode::Esc | KeyCode::Enter => {
                                state.filter_mode = false;
                            }
                            KeyCode::Backspace => {
                                state.proc_filter.pop();
                            }
                            KeyCode::Char(c) => {
                                state.proc_filter.push(c);
                            }
                            _ => {}
                        }
                    } else {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(()),
                            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                return Ok(())
                            }
                            KeyCode::Tab => {
                                state.focus = match state.focus {
                                    Focus::Cpu => Focus::MemNet,
                                    Focus::MemNet => Focus::Proc,
                                    Focus::Proc => Focus::Cpu,
                                }
                            }
                            KeyCode::Char('/') => {
                                state.filter_mode = true;
                                state.proc_filter.clear();
                            }
                            KeyCode::Char('e') => state.expanded = !state.expanded,
                            KeyCode::Char('k') | KeyCode::Delete => state.kill_selected(),
                            KeyCode::Down | KeyCode::Char('j') => state.proc_scroll_down(),
                            KeyCode::Up | KeyCode::Char('i') => state.proc_scroll_up(),
                            KeyCode::F(6) | KeyCode::Char('s') => state.next_sort(),
                            KeyCode::Char('?') | KeyCode::Char('h') => {} // TODO: help overlay
                            _ => {}
                        }
                    }
                }
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollDown => state.proc_scroll_down(),
                    MouseEventKind::ScrollUp => state.proc_scroll_up(),
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

    // Root layout: Top (35%) | Bottom (65%)
    let root_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(area);

    // Top section: Left (40%) | Right (60%)
    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(root_chunks[0]);

    // Top-Left section: CPU Graph (50%) | GPU Graph (50%)
    let top_left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(top_chunks[0]);

    // Bottom section: Left (40%) | Right (60%)
    let bottom_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(root_chunks[1]);

    // Bottom-Left section: Mem/Disk (50%) | Net (50%)
    let bottom_left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(bottom_chunks[0]);

    // Bottom-Left-Top section: Mem (50%) | Disk (50%)
    let mem_disk_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(bottom_left_chunks[0]);

    draw_cpu_graph(f, state, top_left_chunks[0]);
    draw_gpu_box(f, state, top_left_chunks[1]);
    draw_cpu_cores(f, state, top_chunks[1]);

    draw_mem(f, state, mem_disk_chunks[0]);
    draw_disk(f, state, mem_disk_chunks[1]);
    draw_net(f, state, bottom_left_chunks[1]);

    draw_proc(f, state, bottom_chunks[1]);
}

// ── CPU panel ─────────────────────────────────────────────────────────────────

fn draw_cpu_graph(f: &mut Frame, state: &AppState, area: Rect) {
    let t = &state.theme;
    let cpu = &state.cpu;

    let title_style = Style::default()
        .fg(t.header_fg)
        .add_modifier(Modifier::BOLD);
    let block = Block::default()
        .title(Span::styled(
            format!(" cpu {} ", truncate(&cpu.cpu_name, 20)),
            title_style,
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(t.accent));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let uptime_h = (cpu.uptime_seconds / 3600.0) as u64;
    let uptime_m = ((cpu.uptime_seconds % 3600.0) / 60.0) as u64;
    let header = format!(
        "Usage: {:>5.1}%  Load: {:>4.2} {:>4.2} {:>4.2}  Up: {}h{}m",
        cpu.usage_percent, cpu.load_avg[0], cpu.load_avg[1], cpu.load_avg[2], uptime_h, uptime_m
    );

    let usage_color = pct_color(t, cpu.usage_percent);
    let header_widget = Paragraph::new(header).style(Style::default().fg(t.text));

    let sparkline = Sparkline::default()
        .data(&state.cpu_history)
        .max(100)
        .style(Style::default().fg(usage_color));

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(inner);

    f.render_widget(header_widget, rows[0]);
    f.render_widget(sparkline, rows[1]);
}

fn draw_gpu_box(f: &mut Frame, state: &AppState, area: Rect) {
    let t = &state.theme;

    let block = Block::default()
        .title(Span::styled(
            " gpu ",
            Style::default().fg(t.header_fg).bold(),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(t.accent));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut current_usage = 0.0_f64;

    let info = if let Some(igpu) = &state.intel_gpu {
        let render_eng = igpu
            .engines
            .iter()
            .find(|e| e.name.contains("render"))
            .map(|e| e.busy_pct)
            .unwrap_or(0.0);
        current_usage = render_eng;
        format!(
            "Intel GPU | {:.1}W | {}MHz",
            igpu.power_gpu_watts, igpu.freq_act_mhz
        )
    } else if let Some(ngpu) = state.nvidia_gpus.first() {
        current_usage = ngpu.usage_percent;
        format!(
            "{} | {:.1}W | {}°C",
            truncate(&ngpu.name, 15),
            ngpu.power_watts,
            ngpu.temp_c
        )
    } else if let Some(agpu) = state.amd_gpus.first() {
        current_usage = agpu.usage_percent;
        format!(
            "{} | {:.1}W | {}°C",
            truncate(&agpu.name, 15),
            agpu.power_watts,
            agpu.temp_c
        )
    } else {
        "No GPU detected".to_string()
    };

    let header = format!("Usage: {:>5.1}%  {}", current_usage, info);
    let header_widget = Paragraph::new(header).style(Style::default().fg(t.text));

    let sparkline = Sparkline::default()
        .data(&state.gpu_history)
        .max(100)
        .style(Style::default().fg(pct_color(t, current_usage)));

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(inner);

    f.render_widget(header_widget, rows[0]);
    f.render_widget(sparkline, rows[1]);
}

fn draw_cpu_cores(f: &mut Frame, state: &AppState, area: Rect) {
    let t = &state.theme;
    let cpu = &state.cpu;

    let freq_ghz = cpu.freq_mhz.first().copied().unwrap_or(0) as f64 / 1000.0;

    let block = Block::default()
        .title_top(
            ratatui::text::Line::from(format!(" {} ", cpu.cpu_name))
                .alignment(ratatui::layout::Alignment::Left),
        )
        .title_top(
            ratatui::text::Line::from(format!(" {:.1} GHz ", freq_ghz))
                .alignment(ratatui::layout::Alignment::Right),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(t.dim));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let mut lines: Vec<Line> = Vec::new();

    // Overall CPU row
    let cpu_color = pct_color(t, cpu.usage_percent);
    let bar_width = (inner.width as usize).saturating_sub(35).max(10).min(30);
    let spark_width = (inner.width as usize)
        .saturating_sub(bar_width + 35)
        .min(20)
        .max(5);

    let cpu_bar = build_mini_bar(cpu.usage_percent, bar_width);
    let cpu_spark = build_sparkline(&state.cpu_history, spark_width);

    lines.push(Line::from(vec![
        Span::styled(
            "CPU ",
            Style::default().fg(t.text).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("[{cpu_bar}] "), Style::default().fg(cpu_color)),
        Span::styled(
            format!("{:>3.0}% ", cpu.usage_percent),
            Style::default().fg(cpu_color),
        ),
        Span::styled(
            format!("{cpu_spark} "),
            Style::default().fg(Color::Rgb(100, 180, 255)),
        ),
        Span::styled(
            format!("{:>2}°C ", cpu.package_temp_c),
            Style::default().fg(t.text),
        ),
        Span::styled(
            format!("{:>6.1}W", cpu.power_watts),
            Style::default().fg(t.dim),
        ),
    ]));

    // Cores (2 columns)
    let cores = &cpu.cores_percent;
    let col_width = (inner.width as usize) / 2;

    let num_rows = (cores.len() + 1) / 2;
    for r in 0..num_rows {
        let i = r;
        let mut row_spans = Vec::new();

        let add_core = |idx: usize, spans: &mut Vec<Span<'static>>| {
            let pct = cores[idx];
            let c_color = pct_color(t, pct);
            let c_bar_w = col_width.saturating_sub(23).max(5).min(15);
            let c_spark_w = col_width.saturating_sub(c_bar_w + 23).max(5).min(10);

            let bar = build_mini_bar(pct, c_bar_w);
            let spark = build_sparkline(&state.core_history[idx], c_spark_w);
            let temp = cpu.core_temps_c.get(idx).copied().unwrap_or(0);

            spans.push(Span::styled(
                format!("C{idx:<2} "),
                Style::default().fg(t.dim),
            ));
            spans.push(Span::styled(
                format!("[{bar}] "),
                Style::default().fg(c_color),
            ));
            spans.push(Span::styled(
                format!("{:>3.0}% ", pct),
                Style::default().fg(c_color),
            ));
            spans.push(Span::styled(
                format!("{spark} "),
                Style::default().fg(Color::Rgb(100, 180, 255)),
            ));
            if temp > 0 {
                spans.push(Span::styled(
                    format!("{:>2}°C ", temp),
                    Style::default().fg(t.text),
                ));
            } else {
                spans.push(Span::styled("     ", Style::default()));
            }
        };

        add_core(i, &mut row_spans);

        if i + num_rows < cores.len() {
            let j = i + num_rows;
            row_spans.push(Span::styled(" │ ", Style::default().fg(t.dim)));
            add_core(j, &mut row_spans);
        }

        lines.push(Line::from(row_spans));
    }

    // Load Avg
    lines.push(Line::from(vec![Span::styled(
        format!(
            "         Load avg: {:>4.2} {:>4.2} {:>4.2}",
            cpu.load_avg[0], cpu.load_avg[1], cpu.load_avg[2]
        ),
        Style::default().fg(t.dim),
    )]));

    // GPU info logic
    let mut gpu_pct = 0.0_f64;
    let mut gpu_name = "";
    let mut gpu_watts = 0.0_f64;

    if let Some(igpu) = &state.intel_gpu {
        gpu_pct = igpu
            .engines
            .iter()
            .find(|e| e.name.contains("render"))
            .map(|e| e.busy_pct)
            .unwrap_or(0.0);
        gpu_name = "GPU";
        gpu_watts = igpu.power_gpu_watts;
    } else if let Some(ngpu) = state.nvidia_gpus.first() {
        gpu_pct = ngpu.usage_percent;
        gpu_name = "GPU";
        gpu_watts = ngpu.power_watts;
    } else if let Some(agpu) = state.amd_gpus.first() {
        gpu_pct = agpu.usage_percent;
        gpu_name = "GPU";
        gpu_watts = agpu.power_watts;
    }

    if !gpu_name.is_empty() {
        let gpu_color = pct_color(t, gpu_pct);
        let g_bar_w = (inner.width as usize).saturating_sub(35).max(10).min(30);
        let g_spark_w = (inner.width as usize)
            .saturating_sub(g_bar_w + 35)
            .min(20)
            .max(5);

        let g_bar = build_mini_bar(gpu_pct, g_bar_w);
        let g_spark = build_sparkline(&state.gpu_history, g_spark_w);
        let watts_str = if gpu_watts > 0.0 {
            format!("{:>6.1}W", gpu_watts)
        } else {
            "".to_string()
        };

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(
                format!("{gpu_name:<4} "),
                Style::default().fg(t.text).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("[{g_bar}] "), Style::default().fg(gpu_color)),
            Span::styled(
                format!("{:>3.0}% ", gpu_pct),
                Style::default().fg(gpu_color),
            ),
            Span::styled(
                format!("{g_spark} "),
                Style::default().fg(Color::Rgb(100, 180, 255)),
            ),
            Span::styled("     ", Style::default()), // space analogous to temp
            Span::styled(watts_str, Style::default().fg(t.dim)),
        ]));
    }

    f.render_widget(Paragraph::new(lines), inner);
}

// ── MEM panel ─────────────────────────────────────────────────────────────────

fn draw_mem(f: &mut Frame, state: &AppState, area: Rect) {
    let t = &state.theme;
    let mem = &state.mem;

    let block = Block::default()
        .title(Span::styled(
            " mem ",
            Style::default().fg(t.header_fg).bold(),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(t.accent));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Min(0),
        ])
        .split(inner);

    let ram_pct = if mem.total > 0 {
        (mem.used as f64 / mem.total as f64 * 100.0) as u16
    } else {
        0
    };
    let ram_label = format!("{} / {}", fmt_bytes(mem.used), fmt_bytes(mem.total));
    f.render_widget(
        Paragraph::new("Total Ram").style(Style::default().fg(t.text)),
        rows[0],
    );
    let ram_gauge = Gauge::default()
        .block(Block::default().borders(Borders::NONE))
        .gauge_style(
            Style::default()
                .fg(pct_color(t, ram_pct as f64))
                .bg(Color::Rgb(30, 30, 40)),
        )
        .percent(ram_pct)
        .label(ram_label);
    f.render_widget(ram_gauge, rows[1]);

    let swap_pct = if mem.swap_total > 0 {
        (mem.swap_used as f64 / mem.swap_total as f64 * 100.0) as u16
    } else {
        0
    };
    let swap_label = format!(
        "{} / {}",
        fmt_bytes(mem.swap_used),
        fmt_bytes(mem.swap_total)
    );
    f.render_widget(
        Paragraph::new("Total Swap").style(Style::default().fg(t.text)),
        rows[2],
    );
    let swap_gauge = Gauge::default()
        .block(Block::default().borders(Borders::NONE))
        .gauge_style(
            Style::default()
                .fg(pct_color(t, swap_pct as f64))
                .bg(Color::Rgb(30, 30, 40)),
        )
        .percent(swap_pct)
        .label(swap_label);
    f.render_widget(swap_gauge, rows[3]);
}

// ── DISK panel ─────────────────────────────────────────────────────────────────

fn draw_disk(f: &mut Frame, state: &AppState, area: Rect) {
    let t = &state.theme;

    let block = Block::default()
        .title(Span::styled(
            " disks ",
            Style::default().fg(t.header_fg).bold(),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(t.accent));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();
    for dio in &state.disks_io {
        lines.push(Line::from(vec![
            Span::styled(
                format!("{:<8} ", dio.device),
                Style::default().fg(Color::Rgb(180, 140, 255)),
            ),
            Span::styled("R ", Style::default().fg(t.good)),
            Span::styled(
                fmt_bytes_per_sec(dio.read_bytes_per_sec),
                Style::default().fg(t.text),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("         W ", Style::default().fg(t.warn)),
            Span::styled(
                fmt_bytes_per_sec(dio.write_bytes_per_sec),
                Style::default().fg(t.text),
            ),
        ]));
        lines.push(Line::from(""));
    }

    f.render_widget(Paragraph::new(lines), inner);
}

// ── NET panel ─────────────────────────────────────────────────────────────────

fn draw_net(f: &mut Frame, state: &AppState, area: Rect) {
    let t = &state.theme;

    let block = Block::default()
        .title(Span::styled(
            " net ",
            Style::default().fg(t.header_fg).bold(),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(t.accent));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();
    for iface in &state.net {
        if iface.name == "lo" {
            continue;
        }
        lines.push(Line::from(vec![Span::styled(
            format!(" {:<10} ", iface.name),
            Style::default().fg(t.accent),
        )]));
        lines.push(Line::from(vec![
            Span::styled("   ↑ ", Style::default().fg(t.good)),
            Span::styled(
                fmt_bytes_per_sec(iface.tx_bytes_per_sec),
                Style::default().fg(t.text),
            ),
            Span::styled("   ↓ ", Style::default().fg(Color::Rgb(100, 160, 255))),
            Span::styled(
                fmt_bytes_per_sec(iface.rx_bytes_per_sec),
                Style::default().fg(t.text),
            ),
        ]));
        lines.push(Line::from(""));
    }

    f.render_widget(Paragraph::new(lines), inner);
}

// ── Process table ─────────────────────────────────────────────────────────────

fn draw_proc(f: &mut Frame, state: &mut AppState, area: Rect) {
    let t = &state.theme;
    let sort_label = match state.proc_sort {
        ProcSort::Cpu => "CPU▼",
        ProcSort::Mem => "MEM▼",
        ProcSort::Pid => "PID▼",
        ProcSort::Name => "NAME▼",
        ProcSort::Io => "IO▼",
    };
    let filter_str = if state.filter_mode {
        format!(" Filter: {}█", state.proc_filter)
    } else if !state.proc_filter.is_empty() {
        format!(" Filter: {}", state.proc_filter)
    } else {
        String::new()
    };

    let title = format!(
        " proc ({sort_label}){filter_str}  [/]=filter [s]=sort [k]=kill [Tab]=focus [q]=quit "
    );
    let block = Block::default()
        .title(Span::styled(title, Style::default().fg(t.header_fg).bold()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if state.focus == Focus::Proc {
            t.accent
        } else {
            t.dim
        }));

    let header_cells = [
        "PID", "NAME", "CMD", "CPU%", "MEM%", "MEM RSS", "IO R/s", "IO W/s",
    ]
    .iter()
    .map(|h| {
        Cell::from(*h).style(
            Style::default()
                .fg(t.header_fg)
                .add_modifier(Modifier::BOLD),
        )
    });
    let header = Row::new(header_cells).height(1).bottom_margin(0);

    let rows: Vec<Row> = state
        .procs
        .iter()
        .map(|p| {
            let cpu_color = pct_color(t, p.cpu_percent);
            Row::new(vec![
                Cell::from(p.pid.to_string()).style(Style::default().fg(t.dim)),
                Cell::from(truncate(&p.name, 16))
                    .style(Style::default().fg(t.text).add_modifier(Modifier::BOLD)),
                Cell::from(truncate(&p.cmdline, 24)).style(Style::default().fg(t.dim)),
                Cell::from(format!("{:.1}", p.cpu_percent)).style(Style::default().fg(cpu_color)),
                Cell::from(format!("{:.1}", p.mem_percent))
                    .style(Style::default().fg(pct_color(t, p.mem_percent))),
                Cell::from(fmt_bytes(p.mem_rss_bytes)).style(Style::default().fg(t.text)),
                Cell::from(fmt_bytes_per_sec(p.io_read_bytes as f64))
                    .style(Style::default().fg(t.good)),
                Cell::from(fmt_bytes_per_sec(p.io_write_bytes as f64))
                    .style(Style::default().fg(t.warn)),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(7),  // PID
            Constraint::Length(17), // NAME
            Constraint::Min(15),    // CMD
            Constraint::Length(6),  // CPU%
            Constraint::Length(6),  // MEM%
            Constraint::Length(9),  // MEM RSS
            Constraint::Length(9),  // IO R
            Constraint::Length(9),  // IO W
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
    if pct >= 90.0 {
        t.critical
    } else if pct >= 70.0 {
        t.warn
    } else {
        t.good
    }
}

fn build_mini_bar(pct: f64, width: usize) -> String {
    let filled = ((pct / 100.0) * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

fn build_sparkline(data: &[u64], width: usize) -> String {
    let chars = [' ', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let mut s = String::with_capacity(width);
    let start = data.len().saturating_sub(width);
    let slice = if data.len() < width {
        data
    } else {
        &data[start..]
    };
    for &val in slice {
        let idx = ((val as f64 / 100.0) * 7.0).round() as usize;
        s.push(chars[idx.min(7)]);
    }
    while s.chars().count() < width {
        s.insert(0, ' ');
    }
    s
}

fn fmt_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB"];
    let mut val = bytes as f64;
    let mut unit = 0;
    while val >= 1024.0 && unit + 1 < UNITS.len() {
        val /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes}B")
    } else {
        format!("{val:.1}{}", UNITS[unit])
    }
}

fn fmt_bytes_per_sec(bps: f64) -> String {
    const UNITS: &[&str] = &["B/s", "KB/s", "MB/s", "GB/s"];
    let mut val = bps;
    let mut unit = 0;
    while val >= 1024.0 && unit + 1 < UNITS.len() {
        val /= 1024.0;
        unit += 1;
    }
    format!("{val:6.1}{}", UNITS[unit])
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max - 1])
    }
}
