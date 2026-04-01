use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::commands::tui::{
    state::AppState,
    utils::{build_bar, fmt_bytes, truncate},
};

pub fn draw(f: &mut Frame, state: &AppState, area: Rect) {
    if area.height == 0 || area.width == 0 { return; }

    let t = &state.theme;

    let block = Block::default()
        .title(" GPU Detail ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(t.border));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 { return; }

    let mut lines = Vec::new();

    if let Some(igpu) = &state.intel_gpu {
        let render_eng = igpu.engines.iter().find(|e| e.name.contains("render")).map(|e| e.busy_pct).unwrap_or(0.0);
        let bar_w = (inner.width as usize).saturating_sub(20).clamp(5, 20);
        let bar_str = build_bar(render_eng, bar_w);

        lines.push(Line::from(vec![Span::styled(
            if igpu.name.is_empty() { "Intel GPU".to_string() } else { truncate(&igpu.name, inner.width as usize) },
            Style::default().fg(t.title).add_modifier(Modifier::BOLD)
        )]));

        lines.push(Line::from(vec![
            Span::styled("Core:  ", Style::default().fg(t.dim)),
            Span::styled(format!("[{bar_str}] {:>3.0}%", render_eng), Style::default().fg(t.pct_color(render_eng))),
        ]));

        lines.push(Line::from(vec![
            Span::styled(format!("Freq: {:.0}MHz  Pwr: {:.1}W", igpu.freq_act_mhz, igpu.power_gpu_watts), Style::default().fg(t.dim)),
        ]));

    } else if let Some(ngpu) = state.nvidia_gpus.first() {
        let bar_w = (inner.width as usize).saturating_sub(20).clamp(5, 20);
        let bar_str = build_bar(ngpu.usage_percent, bar_w);
        let mem_pct = if ngpu.mem_total > 0 { ngpu.mem_used as f64 / ngpu.mem_total as f64 * 100.0 } else { 0.0 };
        let mem_bar_str = build_bar(mem_pct, bar_w);

        lines.push(Line::from(vec![Span::styled(
            truncate(&ngpu.name, inner.width as usize),
            Style::default().fg(t.title).add_modifier(Modifier::BOLD)
        )]));

        lines.push(Line::from(vec![
            Span::styled(format!("VRAM: {} / {} ", fmt_bytes(ngpu.mem_used), fmt_bytes(ngpu.mem_total)), Style::default().fg(t.text)),
            Span::styled(format!("[{mem_bar_str}] {:>3.0}%", mem_pct), Style::default().fg(t.pct_color(mem_pct))),
        ]));

        lines.push(Line::from(vec![
            Span::styled("Core: ", Style::default().fg(t.dim)),
            Span::styled(format!("[{bar_str}] {:>3.0}%", ngpu.usage_percent), Style::default().fg(t.pct_color(ngpu.usage_percent))),
            Span::styled(format!("  {}°C", ngpu.temp_c), Style::default().fg(t.text)),
        ]));

        lines.push(Line::from(vec![
            Span::styled(format!("Pwr: {:.1}W  Freq: {}MHz", ngpu.power_watts, ngpu.freq_mhz), Style::default().fg(t.dim)),
        ]));

    } else if let Some(agpu) = state.amd_gpus.first() {
        let bar_w = (inner.width as usize).saturating_sub(20).clamp(5, 20);
        let bar_str = build_bar(agpu.usage_percent, bar_w);
        let mem_pct = if agpu.mem_total > 0 { agpu.mem_used as f64 / agpu.mem_total as f64 * 100.0 } else { 0.0 };
        let mem_bar_str = build_bar(mem_pct, bar_w);

        lines.push(Line::from(vec![Span::styled(
            truncate(&agpu.name, inner.width as usize),
            Style::default().fg(t.title).add_modifier(Modifier::BOLD)
        )]));

        lines.push(Line::from(vec![
            Span::styled(format!("VRAM: {} / {} ", fmt_bytes(agpu.mem_used), fmt_bytes(agpu.mem_total)), Style::default().fg(t.text)),
            Span::styled(format!("[{mem_bar_str}] {:>3.0}%", mem_pct), Style::default().fg(t.pct_color(mem_pct))),
        ]));

        lines.push(Line::from(vec![
            Span::styled("Core: ", Style::default().fg(t.dim)),
            Span::styled(format!("[{bar_str}] {:>3.0}%", agpu.usage_percent), Style::default().fg(t.pct_color(agpu.usage_percent))),
            Span::styled(format!("  {:.0}°C", agpu.temp_c), Style::default().fg(t.text)),
        ]));

        lines.push(Line::from(vec![
            Span::styled(format!("Pwr: {:.1}W  Freq: {}MHz", agpu.power_watts, agpu.freq_mhz), Style::default().fg(t.dim)),
        ]));

    } else {
        lines.push(Line::from(Span::styled("No GPU detected", Style::default().fg(t.dim))));
    }

    f.render_widget(Paragraph::new(lines), inner);
}
