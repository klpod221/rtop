use ratatui::layout::{Constraint, Direction, Layout, Rect};
use super::state::AppState;

pub struct TuiLayout {
    pub menu_bar: Rect,
    pub cpu_graph: Rect,
    pub gpu_graph: Rect,
    pub cpu_detail: Rect,
    pub gpu_detail: Rect,
    pub mem: Rect,
    pub disks: Rect,
    pub net: Rect,
    pub proc: Rect,
}

pub fn build_layout(area: Rect, state: &AppState) -> TuiLayout {
    let has_gpu = state.intel_gpu.is_some() || !state.nvidia_gpus.is_empty() || !state.amd_gpus.is_empty();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);
        
    let menu_bar = chunks[0];
    let main_area = chunks[1];

    let row_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(main_area);

    let top_row = row_chunks[0];
    let bottom_row = row_chunks[1];

    // TOP ROW
    let top_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(top_row);

    let tl = top_cols[0];
    let tr = top_cols[1];

    let (cpu_graph, gpu_graph) = if has_gpu {
        let splits = Layout::default().direction(Direction::Vertical)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)]).split(tl);
        (splits[0], splits[1])
    } else {
        (tl, Rect::default())
    };

    let (cpu_detail, gpu_detail) = if has_gpu {
        let splits = Layout::default().direction(Direction::Vertical)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)]).split(tr);
        (splits[0], splits[1])
    } else {
        (tr, Rect::default())
    };

    // BOTTOM ROW
    let bottom_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(bottom_row);
        
    let bl = bottom_cols[0];
    let proc = bottom_cols[1];

    let bl_splits = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(bl);
        
    let bl_top = bl_splits[0];
    let net = bl_splits[1];

    let bl_top_splits = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(bl_top);

    let mem = bl_top_splits[0];
    let disks = bl_top_splits[1];

    TuiLayout {
        menu_bar,
        cpu_graph,
        gpu_graph,
        cpu_detail,
        gpu_detail,
        mem,
        disks,
        net,
        proc,
    }
}
