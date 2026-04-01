use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Paragraph},
    Frame,
};

use crate::commands::tui::state::AppState;

pub fn draw(f: &mut Frame, state: &AppState, area: Rect) {
    if area.height == 0 { return; }

    let t = &state.theme;
    
    let mut left_spans = vec![
        Span::styled(" cpu ", Style::default().fg(t.title)),
        Span::styled(" ▼menu ", Style::default().fg(t.dim)),
        Span::styled(" ▼preset ", Style::default().fg(t.dim)),
    ];
    if state.filter_mode || !state.proc_filter.is_empty() {
        left_spans.push(Span::styled(" *", Style::default().fg(t.accent)));
    }
    let left = Line::from(left_spans);

    let center = Line::from(vec![Span::styled(
        chrono::Local::now().format("%H:%M:%S").to_string(),
        Style::default().fg(t.text),
    )]).alignment(ratatui::layout::Alignment::Center);

    let right = Line::from(vec![Span::styled(
        format!("{}ms ▼ ", state.refresh_ms),
        Style::default().fg(t.dim),
    )]).alignment(ratatui::layout::Alignment::Right);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(30), Constraint::Min(20), Constraint::Length(20)])
        .split(area);

    f.render_widget(Paragraph::new(left).block(Block::default()), chunks[0]);
    f.render_widget(Paragraph::new(center).block(Block::default()), chunks[1]);
    f.render_widget(Paragraph::new(right).block(Block::default()), chunks[2]);
}
