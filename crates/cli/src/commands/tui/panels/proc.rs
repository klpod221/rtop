use ratatui::{
    layout::Constraint,
    layout::Rect,
    style::{Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Row, Table},
    Frame,
};

use std::collections::HashMap;

use crate::commands::tui::{
    state::{AppState, Focus, ProcSort},
    utils::{fmt_bytes, truncate},
};

pub fn draw(f: &mut Frame, state: &mut AppState, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let t = &state.theme;

    let border_style = if state.focus == Focus::Proc {
        Style::default().fg(t.border_focused)
    } else {
        Style::default().fg(t.border)
    };

    let title = ratatui::text::Line::from(vec![
        Span::styled(" proc ▼ ", Style::default().fg(t.title)),
        Span::styled(" filter ", Style::default().fg(t.dim)),
        Span::styled(
            if state.per_core_mode {
                " per-core▼ "
            } else {
                " per-core□ "
            },
            Style::default().fg(t.dim),
        ),
        Span::styled(
            if state.reverse_sort {
                " reverse▼ "
            } else {
                " reverse□ "
            },
            Style::default().fg(t.dim),
        ),
        Span::styled(
            if state.tree_mode {
                " tree▼ "
            } else {
                " tree□ "
            },
            Style::default().fg(t.dim),
        ),
    ])
    .alignment(ratatui::layout::Alignment::Left);

    let block = Block::default()
        .title_top(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    // Build column headers with sort indicator
    let col_names = [
        "Pid", "Program", "Command", "Threads", "User", "MemB", "Cpu%",
    ];
    let sort_idx = match state.proc_sort {
        ProcSort::Pid => 0,
        ProcSort::Name => 1,
        ProcSort::Threads => 3,
        ProcSort::Mem => 5,
        ProcSort::Cpu => 6,
    };
    let indicator = if state.reverse_sort { "▲" } else { "▼" };
    let header_strs: Vec<String> = col_names
        .iter()
        .enumerate()
        .map(|(i, &name)| {
            if i == sort_idx {
                format!("{}{}", name, indicator)
            } else {
                name.to_string()
            }
        })
        .collect();

    let header = Row::new(header_strs)
        .style(Style::default().fg(t.text).add_modifier(Modifier::BOLD))
        .bottom_margin(0);

    // Build display items: (prefix, index into state.procs)
    let display_items: Vec<(String, usize)> = if state.tree_mode {
        build_tree_items(&state.procs)
    } else {
        (0..state.procs.len()).map(|i| (String::new(), i)).collect()
    };

    let rows: Vec<Row> = display_items
        .iter()
        .map(|(prefix, i)| {
            let p = &state.procs[*i];
            Row::new(vec![
                p.pid.to_string(),
                format!("{}{}", prefix, truncate(&p.name, 15)),
                truncate(&p.cmdline, 40),
                p.threads.to_string(),
                truncate(&p.user, 10),
                fmt_bytes(p.mem_rss_bytes),
                format!("{:.1}", p.cpu_percent),
            ])
        })
        .collect();

    let selected_style = Style::default().bg(t.selected_bg);
    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Length(25),
            Constraint::Min(20),
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(8),
        ],
    )
    .header(header)
    .block(block)
    .row_highlight_style(selected_style);

    f.render_stateful_widget(table, area, &mut state.proc_table);
}

fn build_tree_items(procs: &[app_collector::proc::ProcessInfo]) -> Vec<(String, usize)> {
    // Map pid -> index
    let pid_to_idx: HashMap<u32, usize> =
        procs.iter().enumerate().map(|(i, p)| (p.pid, i)).collect();

    // Build children map: ppid -> [child_indices]
    let mut children: HashMap<u32, Vec<usize>> = HashMap::new();
    let mut roots: Vec<usize> = Vec::new();

    for (i, p) in procs.iter().enumerate() {
        if pid_to_idx.contains_key(&p.ppid) && p.ppid != p.pid {
            children.entry(p.ppid).or_default().push(i);
        } else {
            roots.push(i);
        }
    }

    let mut out = Vec::new();
    let mut stack: Vec<(usize, String, bool)> = Vec::new();

    // Push roots in reverse so we pop in order
    for &i in roots.iter().rev() {
        stack.push((i, String::new(), true));
    }

    while let Some((idx, prefix, _)) = stack.pop() {
        out.push((prefix.clone(), idx));

        let pid = procs[idx].pid;
        if let Some(ch) = children.get(&pid) {
            let n = ch.len();
            for (ci, &child_i) in ch.iter().enumerate().rev() {
                let is_last = ci == n - 1;
                let connector = if is_last { "└─ " } else { "├─ " };
                // Convert existing connectors to space/bar
                let new_prefix = prefix.replace("├─ ", "│  ").replace("└─ ", "   ") + connector;
                stack.push((child_i, new_prefix, is_last));
            }
        }
    }

    out
}
