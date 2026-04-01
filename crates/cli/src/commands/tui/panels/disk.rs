use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::commands::tui::{
    state::{AppState, Focus},
    utils::{build_bar, fmt_bytes, truncate},
};

pub fn draw(f: &mut Frame, state: &AppState, area: Rect) {
    if area.height == 0 || area.width == 0 { return; }

    let t = &state.theme;

    let border_style = if state.focus == Focus::Disk {
        Style::default().fg(t.border_focused)
    } else {
        Style::default().fg(t.border)
    };

    let title1 = ratatui::text::Line::from(vec![
        Span::styled(" disks ", Style::default().fg(t.title)),
    ]).alignment(ratatui::layout::Alignment::Left);
    let title2 = ratatui::text::Line::from(" io ").alignment(ratatui::layout::Alignment::Right);

    let block = Block::default()
        .title_top(title1)
        .title_top(title2)
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 { return; }

    let mut lines = Vec::new();

    for d in &state.disks_space {
        if d.total == 0 || d.mount_point.starts_with("/dev") || d.mount_point.starts_with("/sys") || d.mount_point.starts_with("/proc") {
            continue;
        }
        let pct = if d.total > 0 { d.used as f64 / d.total as f64 * 100.0 } else { 0.0 };
        let bar_w = (inner.width as usize).saturating_sub(15).max(5);
        let bar = build_bar(pct, bar_w);
        
        let t_mount = truncate(&d.mount_point, 10);
        let w_space = (inner.width as usize).saturating_sub(t_mount.chars().count()).max(1);

        lines.push(Line::from(vec![
            Span::styled(t_mount, Style::default().fg(t.text).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{:>width$}", fmt_bytes(d.total), width = w_space), Style::default().fg(t.text)),
        ]));
        
        lines.push(Line::from(vec![
            Span::styled("IO%: ", Style::default().fg(t.dim)),
            Span::styled(" ".repeat(10), Style::default()), // sparkline place
            Span::styled(" :", Style::default().fg(t.dim)),
        ]));

        lines.push(Line::from(vec![
            Span::styled("Used: ", Style::default().fg(t.dim)),
            Span::styled(bar, Style::default().fg(t.pct_color(pct))),
            Span::styled(format!("{:>3.0}%", pct), Style::default().fg(t.pct_color(pct))),
        ]));
        
        lines.push(Line::from(vec![
            Span::styled(format!("{:>width$}", fmt_bytes(d.used), width = inner.width as usize), Style::default().fg(t.text)),
        ]));
    }

    f.render_widget(Paragraph::new(lines), inner);
}
