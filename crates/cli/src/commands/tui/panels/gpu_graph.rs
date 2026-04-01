use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::Span,
    widgets::{Block, Borders, Chart, Dataset, GraphType},
    symbols,
    Frame,
};

use crate::commands::tui::state::AppState;

pub fn draw(f: &mut Frame, state: &AppState, area: Rect) {
    if area.height == 0 || area.width == 0 { return; }
    
    let t = &state.theme;

    let title = ratatui::text::Line::from(vec![
        Span::styled(" gpu ▼ ", Style::default().fg(t.title)),
    ]);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(t.border));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 { return; }

    let mut current_usage = 0.0;
    if let Some(igpu) = &state.intel_gpu {
        current_usage = igpu.engines.iter().find(|e| e.name.contains("render")).map(|e| e.busy_pct).unwrap_or(0.0);
    } else if let Some(ngpu) = state.nvidia_gpus.first() {
        current_usage = ngpu.usage_percent;
    } else if let Some(agpu) = state.amd_gpus.first() {
        current_usage = agpu.usage_percent;
    }

    let top_label = ratatui::text::Line::from(format!("Usage: {:.1}%", current_usage));

    let mut data = Vec::with_capacity(state.gpu_history.len());
    let mut x = 0.0;
    let width = inner.width as usize * 2;
    let offset = state.gpu_history.len().saturating_sub(width);
    
    for i in offset..state.gpu_history.len() {
        data.push((x, state.gpu_history[i] as f64));
        x += 1.0;
    }

    let dataset = Dataset::default()
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(t.pct_color(current_usage)))
        .data(&data);

    let chart = Chart::new(vec![dataset])
        .x_axis(ratatui::widgets::Axis::default().bounds([0.0, (width as f64 - 1.0).max(1.0)]))
        .y_axis(ratatui::widgets::Axis::default().bounds([0.0, 100.0]));

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(inner);

    f.render_widget(ratatui::widgets::Paragraph::new(top_label), layout[0]);
    f.render_widget(chart, layout[1]);
}
