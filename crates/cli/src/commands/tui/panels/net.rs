use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Chart, Dataset, GraphType, Paragraph},
    symbols,
    Frame,
};

use crate::commands::tui::{
    state::{AppState, Focus},
    utils::{fmt_bps, fmt_bytes},
};

pub fn draw(f: &mut Frame, state: &AppState, area: Rect) {
    if area.height == 0 || area.width == 0 { return; }

    let t = &state.theme;
    let border_style = if state.focus == Focus::Net {
        Style::default().fg(t.border_focused)
    } else {
        Style::default().fg(t.border)
    };

    // Title: left = " net · <ip> ", right = "<b iface n>"
    let iface_label = if let Some(iface) = state.net.get(state.selected_iface) {
        format!(" <b {} n> ", iface.name)
    } else {
        " <b -- n> ".to_string()
    };

    let block = Block::default()
        .title_top(
            Line::from(vec![Span::styled(" net ", Style::default().fg(t.title))])
                .alignment(ratatui::layout::Alignment::Left),
        )
        .title_top(
            Line::from(Span::styled(iface_label, Style::default().fg(t.dim)))
                .alignment(ratatui::layout::Alignment::Right),
        )
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 2 || inner.width < 10 { return; }

    // Horizontal split: graphs left (~60%), stats right (~40%)
    let h_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(inner);

    let graph_area = h_chunks[0];
    let stats_area = h_chunks[1];

    // Graph area: RX top half, TX bottom half (equal split)
    let graph_splits = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(graph_area);

    let rx_area = graph_splits[0];
    let tx_area = graph_splits[1];

    let width = graph_area.width as usize * 2; // braille dots per char = 2 horizontal

    // ── RX Graph ──────────────────────────────────────────────────────────────
    let mut rx_data: Vec<(f64, f64)> = Vec::with_capacity(width);
    let offset_rx = state.net_rx_history.len().saturating_sub(width);
    for (i, &val) in state.net_rx_history.iter().enumerate().skip(offset_rx) {
        rx_data.push(((i - offset_rx) as f64, val));
    }

    let rx_max = state.net_rx_peak.max(1024.0);
    let rx_dataset = Dataset::default()
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(t.net_rx))
        .data(&rx_data);

    let rx_chart = Chart::new(vec![rx_dataset])
        .x_axis(ratatui::widgets::Axis::default().bounds([0.0, width as f64]))
        .y_axis(
            ratatui::widgets::Axis::default()
                .bounds([0.0, rx_max])
                .labels(vec![
                    Span::raw(""),
                    Span::styled(fmt_bps_short(rx_max), Style::default().fg(t.dim)),
                ]),
        );

    f.render_widget(rx_chart, rx_area);

    // ── TX Graph ──────────────────────────────────────────────────────────────
    let mut tx_data: Vec<(f64, f64)> = Vec::with_capacity(width);
    let offset_tx = state.net_tx_history.len().saturating_sub(width);
    for (i, &val) in state.net_tx_history.iter().enumerate().skip(offset_tx) {
        tx_data.push(((i - offset_tx) as f64, val));
    }

    let tx_max = state.net_tx_peak.max(1024.0);
    let tx_dataset = Dataset::default()
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(t.net_tx))
        .data(&tx_data);

    let tx_chart = Chart::new(vec![tx_dataset])
        .x_axis(ratatui::widgets::Axis::default().bounds([0.0, width as f64]))
        .y_axis(
            ratatui::widgets::Axis::default()
                .bounds([0.0, tx_max])
                .labels(vec![
                    Span::raw(""),
                    Span::styled(fmt_bps_short(tx_max), Style::default().fg(t.dim)),
                ]),
        );

    f.render_widget(tx_chart, tx_area);

    // ── Stats (right side) ────────────────────────────────────────────────────
    if let Some(net) = state.net.get(state.selected_iface) {
        let mut lines: Vec<Line> = Vec::new();

        // Blank lines to vertically center the stats block (height / 2 - 4)
        let pad_top = stats_area.height.saturating_sub(8) / 2;
        for _ in 0..pad_top {
            lines.push(Line::from(""));
        }

        // ── download ──────────────────────────────────────────────────────────
        lines.push(Line::from(vec![
            Span::styled("download", Style::default().fg(t.net_rx)),
        ]));

        lines.push(Line::from(vec![
            Span::styled(" ▼ ", Style::default().fg(t.net_rx)),
            Span::styled(
                format!("{:<12}", fmt_bps(net.rx_bytes_per_sec)),
                Style::default().fg(t.text),
            ),
            Span::styled(
                format!("({})", fmt_bits(net.rx_bytes_per_sec)),
                Style::default().fg(t.dim),
            ),
        ]));

        lines.push(Line::from(vec![
            Span::styled(" ▼ Top:    ", Style::default().fg(t.dim)),
            Span::styled(
                format!("({})", fmt_bits(state.net_rx_peak)),
                Style::default().fg(t.dim),
            ),
        ]));

        lines.push(Line::from(vec![
            Span::styled(" ▼ Total:  ", Style::default().fg(t.dim)),
            Span::styled(fmt_bytes(net.rx_bytes_total), Style::default().fg(t.text)),
        ]));

        lines.push(Line::from(""));

        // ── upload ────────────────────────────────────────────────────────────
        lines.push(Line::from(vec![
            Span::styled("upload", Style::default().fg(t.net_tx)),
        ]));

        lines.push(Line::from(vec![
            Span::styled(" ▲ ", Style::default().fg(t.net_tx)),
            Span::styled(
                format!("{:<12}", fmt_bps(net.tx_bytes_per_sec)),
                Style::default().fg(t.text),
            ),
            Span::styled(
                format!("({})", fmt_bits(net.tx_bytes_per_sec)),
                Style::default().fg(t.dim),
            ),
        ]));

        lines.push(Line::from(vec![
            Span::styled(" ▲ Top:    ", Style::default().fg(t.dim)),
            Span::styled(
                format!("({})", fmt_bits(state.net_tx_peak)),
                Style::default().fg(t.dim),
            ),
        ]));

        lines.push(Line::from(vec![
            Span::styled(" ▲ Total:  ", Style::default().fg(t.dim)),
            Span::styled(fmt_bytes(net.tx_bytes_total), Style::default().fg(t.text)),
        ]));

        f.render_widget(Paragraph::new(lines), stats_area);
    }
}

fn fmt_bits(bytes_per_sec: f64) -> String {
    let bits = bytes_per_sec * 8.0;
    if bits < 1_000.0 {
        format!("{:.0} bits", bits)
    } else if bits < 1_000_000.0 {
        format!("{:.1} Kibps", bits / 1024.0)
    } else if bits < 1_000_000_000.0 {
        format!("{:.2} Mibps", bits / (1024.0 * 1024.0))
    } else {
        format!("{:.2} Gibps", bits / (1024.0 * 1024.0 * 1024.0))
    }
}

fn fmt_bps_short(bps: f64) -> String {
    if bps < 1024.0 { format!("{:.0}B", bps) }
    else if bps < 1024.0 * 1024.0 { format!("{:.0}K", bps / 1024.0) }
    else { format!("{:.1}M", bps / (1024.0 * 1024.0)) }
}
