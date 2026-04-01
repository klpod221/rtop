use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::Span,
    widgets::{Block, Borders, Chart, Dataset, GraphType},
    symbols,
    Frame,
};

use crate::commands::tui::{
    state::{AppState, Focus},
    utils::human_uptime,
};

pub fn draw(f: &mut Frame, state: &AppState, area: Rect) {
    if area.height == 0 { return; }
    
    let t = &state.theme;
    let cpu = &state.cpu;

    let border_style = if state.focus == Focus::CpuGraph {
        Style::default().fg(t.border_focused)
    } else {
        Style::default().fg(t.border)
    };

    let title = ratatui::text::Line::from(vec![
        Span::styled(" cpu ▼ ", Style::default().fg(t.title)),
    ]);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 { return; }

    // Uptime and label
    let uptime_str = human_uptime(cpu.uptime_seconds);
    let top_label = ratatui::text::Line::from(format!("Usage: {:.1}%", cpu.usage_percent));
    let bottom_label = ratatui::text::Line::from(format!("up {}    total ▲ gpu-totals", uptime_str));

    // build dataset
    let width = inner.width as usize * 2;
    let mut data = Vec::with_capacity(width.min(state.cpu_history.len()));
    let mut x = 0.0f64;
    let offset = state.cpu_history.len().saturating_sub(width);

    for i in offset..state.cpu_history.len() {
        data.push((x, state.cpu_history[i] as f64));
        x += 1.0;
    }

    let dataset = Dataset::default()
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(t.pct_color(cpu.usage_percent)))
        .data(&data);

    let chart = Chart::new(vec![dataset])
        .x_axis(ratatui::widgets::Axis::default().bounds([0.0, (width as f64 - 1.0).max(1.0)]))
        .y_axis(ratatui::widgets::Axis::default().bounds([0.0, 100.0]));

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(inner);

    f.render_widget(ratatui::widgets::Paragraph::new(top_label), layout[0]);
    f.render_widget(chart, layout[1]);
    f.render_widget(ratatui::widgets::Paragraph::new(bottom_label), layout[2]);
}
