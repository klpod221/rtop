use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::commands::tui::{
    state::{AppState, Focus},
    utils::{build_bar, single_line_braille_spark, truncate},
};

pub fn draw(f: &mut Frame, state: &AppState, area: Rect) {
    if area.height == 0 || area.width == 0 { return; }

    let t = &state.theme;
    let cpu = &state.cpu;

    let border_style = if state.focus == Focus::CpuDetail {
        Style::default().fg(t.border_focused)
    } else {
        Style::default().fg(t.border)
    };

    let p_freq = ratatui::text::Line::from(format!(" {:.1} GHz ", 
        cpu.freq_mhz.first().copied().unwrap_or(0) as f64 / 1000.0)
    ).alignment(ratatui::layout::Alignment::Right);

    let title1 = ratatui::text::Line::from(format!(" {} ", truncate(&cpu.cpu_name, 20)))
        .alignment(ratatui::layout::Alignment::Left);

    let block = Block::default()
        .title_top(title1)
        .title_top(p_freq)
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 { return; }

    let mut lines = Vec::new();

    // CPU Overall
    let cpu_color = t.pct_color(cpu.usage_percent);
    let bar_w = (inner.width as usize).saturating_sub(30).clamp(5, 20);
    let bar_str = build_bar(cpu.usage_percent, bar_w);
    let spark_w = (inner.width as usize).saturating_sub(bar_w + 30).max(0).min(10);
    let spark_str = if spark_w > 0 {
        single_line_braille_spark(&state.cpu_history, spark_w)
    } else { "".to_string() };

    lines.push(Line::from(vec![
        Span::styled("CPU  ", Style::default().fg(t.text).add_modifier(Modifier::BOLD)),
        Span::styled(format!("[{bar_str}] "), Style::default().fg(cpu_color)),
        Span::styled(format!("{:>3.0}% ", cpu.usage_percent), Style::default().fg(cpu_color)),
        Span::styled(format!("{spark_str} "), Style::default().fg(t.accent)),
        Span::styled(format!("{:>2}°C  ", cpu.package_temp_c), Style::default().fg(t.text)),
        Span::styled(format!("{:.1}W", cpu.power_watts), Style::default().fg(t.dim)),
    ]));

    // Cores
    let cores = &cpu.cores_percent;
    let half_w = (inner.width as usize) / 2;
    let num_rows = (cores.len() + 1) / 2;

    for r in 0..num_rows {
        let mut row_spans = Vec::new();
        
        let mut add_core = |idx: usize| {
            if idx >= cores.len() {
                row_spans.push(Span::styled(" ".repeat(half_w), Style::default()));
                return;
            }
            let pct = cores[idx];
            let c_color = t.pct_color(pct);
            let c_bar_w = half_w.saturating_sub(18).clamp(3, 10);
            let c_bar = build_bar(pct, c_bar_w);
            let c_spark_w = half_w.saturating_sub(c_bar_w + 18).max(0).min(5);
            let c_spark = if c_spark_w > 0 && state.core_history.len() > idx {
                single_line_braille_spark(&state.core_history[idx], c_spark_w)
            } else { "".to_string() };
            
            let temp = cpu.core_temps_c.get(idx).copied().unwrap_or(0);
            
            row_spans.push(Span::styled(format!("C{idx:<2} "), Style::default().fg(t.dim)));
            row_spans.push(Span::styled(format!("[{c_bar}] "), Style::default().fg(c_color)));
            row_spans.push(Span::styled(format!("{:>3.0}% ", pct), Style::default().fg(c_color)));
            if !c_spark.is_empty() {
                row_spans.push(Span::styled(format!("{c_spark} "), Style::default().fg(t.accent)));
            }
            if temp > 0 {
                row_spans.push(Span::styled(format!("{:>2}°C ", temp), Style::default().fg(t.text)));
            } else {
                row_spans.push(Span::styled("     ", Style::default()));
            }
        };

        add_core(r);
        if r + num_rows < cores.len() {
            add_core(r + num_rows);
        }
        lines.push(Line::from(row_spans));
    }

    // Load Avg
    lines.push(Line::from(vec![Span::styled(
        format!(
            "Load avg: {:>4.2} {:>4.2} {:>4.2}",
            cpu.load_avg[0], cpu.load_avg[1], cpu.load_avg[2]
        ),
        Style::default().fg(t.dim),
    )]));

    f.render_widget(Paragraph::new(lines), inner);
}
