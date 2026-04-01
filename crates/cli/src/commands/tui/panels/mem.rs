use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::commands::tui::{
    state::{AppState, Focus},
    utils::{build_bar, fmt_bytes},
};

pub fn draw(f: &mut Frame, state: &AppState, area: Rect) {
    if area.height == 0 || area.width == 0 { return; }

    let t = &state.theme;
    let mem = &state.mem;

    let border_style = if state.focus == Focus::Mem {
        Style::default().fg(t.border_focused)
    } else {
        Style::default().fg(t.border)
    };

    let title1 = ratatui::text::Line::from(vec![
        Span::styled(" mem ", Style::default().fg(t.title)),
    ]);

    let block = Block::default()
        .title(title1)
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 { return; }

    let bar_w = (inner.width as usize).saturating_sub(6).max(5);
    let mut lines = Vec::new();

    let total = mem.total;
    let used = mem.used;
    let avail = total.saturating_sub(used); 
    let cached = mem.cached;
    let free = mem.free;

    let pct_used = if total > 0 { used as f64 / total as f64 * 100.0 } else { 0.0 };
    let pct_avail = if total > 0 { avail as f64 / total as f64 * 100.0 } else { 0.0 };
    let pct_cached = if total > 0 { cached as f64 / total as f64 * 100.0 } else { 0.0 };
    let pct_free = if total > 0 { free as f64 / total as f64 * 100.0 } else { 0.0 };

    let add_metric = |lines: &mut Vec<Line>, name: &str, val: u64, pct: f64, color: ratatui::style::Color| {
        let space = (inner.width as usize).saturating_sub(name.len() + 1).max(1);
        lines.push(Line::from(vec![
            Span::styled(format!("{name}:"), Style::default().fg(t.text)),
            Span::styled(format!("{:>width$}", fmt_bytes(val), width = space), Style::default().fg(t.text)),
        ]));
        let bar = build_bar(pct, bar_w);
        lines.push(Line::from(vec![
            Span::styled(format!("{:>3.0}% ", pct), Style::default().fg(color)),
            Span::styled(bar, Style::default().fg(color)),
        ]));
    };

    let space_total = (inner.width as usize).saturating_sub(7).max(1);
    lines.push(Line::from(vec![
        Span::styled("Total:", Style::default().fg(t.text)),
        Span::styled(format!("{:>width$}", fmt_bytes(total), width = space_total), Style::default().fg(t.text)),
    ]));
    
    add_metric(&mut lines, "Used", used, pct_used, t.mem_used);
    add_metric(&mut lines, "Available", avail, pct_avail, t.good);
    add_metric(&mut lines, "Cached", cached, pct_cached, t.mem_cached);
    add_metric(&mut lines, "Free", free, pct_free, t.dim);

    let sep_len = (inner.width as usize).saturating_sub(7).max(1);
    lines.push(Line::from(Span::styled(format!("─ swap {}", "─".repeat(sep_len)), Style::default().fg(t.border))));

    let swap_pct = if mem.swap_total > 0 { mem.swap_used as f64 / mem.swap_total as f64 * 100.0 } else { 0.0 };
    lines.push(Line::from(vec![
        Span::styled("Total:", Style::default().fg(t.text)),
        Span::styled(format!("{:>width$}", fmt_bytes(mem.swap_total), width = space_total), Style::default().fg(t.text)),
    ]));
    add_metric(&mut lines, "Used", mem.swap_used, swap_pct, t.warn);

    f.render_widget(Paragraph::new(lines), inner);
}
