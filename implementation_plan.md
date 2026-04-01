# Rtop TUI — Redesign Hoàn Toàn Giống Btop

Mục tiêu: viết lại toàn bộ `tui.rs` và tách thành các module con.

---

## Grid Layout (đã xác nhận)

```
┌─────────────────────────────────────────────────┬──────────────────────────────┐
│  CPU Graph  (~70% width)                        │  CPU Detail                  │
│  ─────────────────────────────────────────────  │  per-core bars + temp + freq │
│  GPU Graph  (bên dưới CPU graph)                │  ─────────────────────────── │
│                                                 │  GPU Detail (usage/W/temp)   │
├────────────────┬────────────────────────────────┴──────────────────────────────┤
│  Mem   (50%)   │   Disk  (50%)                  │  Proc table                  │
├────────────────┴────────────────┐               │  (full right height ~60%)    │
│  Network  (full left width)     │               │                              │
└─────────────────────────────────┴───────────────┴──────────────────────────────┘
```

> [!IMPORTANT]
> **Layout Tree chính xác:**
> - **Row 1** (~35% height): Horizontal split
>   - **Left 70%** — Vertical: `cpu_graph` (~60%) + `gpu_graph` (~40%)
>   - **Right 30%** — Vertical: `cpu_detail` (~65%) + `gpu_detail` (~35%)
> - **Row 2** (~65% height): Horizontal split
>   - **Left 40%** — Vertical:
>     - `left_top` (50%) → Horizontal: `mem` (50%) + `disks` (50%)
>     - `net` (50%) — full left width
>   - **Right 60%** — `proc` table (full height)

---

## Top Menu Bar

```
 cpu  ▼menu  ▼preset *              16:58:16                       100ms ▼
```
- Góc trái: `cpu` | `▼menu` | `▼preset`
- Giữa: clock `HH:MM:SS` real-time
- Góc phải: refresh interval `100ms ▼`
- `*` xuất hiện khi có filter active

---

## Panel Chi Tiết

### Panel 1 — CPU Graph (top-left-top, 70%×60%)
- Block title: ` cpu ▼ `
- Sparkline full width với `▂▃▄▅▆▇█`, màu green→yellow→red
- Label bottom: `total ▲ gpu-totals`
- Uptime bottom-left: `up Xd Yh:Zm`

### Panel 2 — GPU Graph (top-left-bottom, 70%×40%)
- Cùng style CPU graph, màu tím `Rgb(120, 90, 200)`
- Nếu không có GPU → "No GPU detected"

### Panel 3 — CPU Detail (top-right-top, 30%×65%)
```
┌─ i5-10400 ──────────────── 800 MHz / 4.0 GHz ─┐
│ CPU  [████████░░░░] 10%  53°C  15.8W           │
│ C0   [█░░░░░░░░░░]  0%  43°C  C6 [████] 42%   │
│ C1   [████░░░░░░░] 25%         C7 [░░░░]  0%   │
│ ...                                             │
│ Load avg: 0.57  0.26  0.41                      │
└─────────────────────────────────────────────────┘
```
- Title: model name trái | `cur MHz / max GHz` phải
- 2 cột core: `C{n} [bar] pct%  tempC`
- Temp bỏ trống nếu = 0

### Panel 4 — GPU Detail (top-right-bottom, 30%×35%)
```
┌─ GPU ──────────────── 10%  0°C  18.0W ─┐
│ Intel Arc / RTX xxxx / RX xxxx          │
│ VRAM: 2.1 / 16.0 GiB  [████░░░░] 13%  │
│ Freq: 800MHz  Pwr: 18W  Temp: 0°C      │
└─────────────────────────────────────────┘
```
- Ưu tiên: Intel → Nvidia → AMD
- Nếu không có GPU: "No GPU detected"

### Panel 5 — Memory (bottom-left-top-left, 20%×50%)
```
┌─ mem ───────────────────────────────┐
│ Total:                   15.4 GiB  │
│ Used:                     8.28 GiB │
│  53% ████████████████░░░░░░░░░░░░  │
│ Available:                7.19 GiB │
│  47% ██████████████████░░░░░░░░░░  │
│ Cached:                   6.59 GiB │
│  43% █████████████░░░░░░░░░░░░░░░  │
│ Free:                     1.36 GiB │
│   9% ███░░░░░░░░░░░░░░░░░░░░░░░░░  │
│─ swap ──────────────────────────── │
│ Total:                   24.7 GiB  │
│ Used:                     2.87 GiB │
│  12% ████░░░░░░░░░░░░░░░░░░░░░░░░  │
└─────────────────────────────────────┘
```
- Mỗi metric: label+value line → bar line
- Tự vẽ bar bằng `▐█░▌` chars (không dùng Gauge widget)
- Swap section với separator `─ swap ────`

### Panel 6 — Disks (bottom-left-top-right, 20%×50%)
```
┌─ disks ───────────────────── io ─┐
│ root                    428 GiB  │
│ IO%: ▁▁▄░▁             :        │
│ Used: ████████████░░░░   82%     │
│       352 GiB                    │
│ efi                    1.99 GiB  │
│ IO%: ░░░░░░           :          │
│ Used: ░░░░░░░░░░░░░░░░   6%      │
│       115 MiB                    │
└───────────────────────────────────┘
```
- `io` trong title bar (không phải panel riêng)
- Mỗi disk: name+size → IO sparkline → Used bar+%+bytes
- Bỏ qua tmpfs size=0, /proc, /sys, /dev

### Panel 7 — Network (bottom-left-bottom, 40%×50%)
```
┌─ net ─ 192.168.1.110 ──── sync▽ auto▽ zero▽ <b enp1s0 n> ─┐
│ [download sparkline - blue]                                   │
│────────────────────────────────────────────────────────────── │
│ [upload sparkline - green]                                    │
│              download                                         │
│ ▼ 594 Byte/s      (4.64 Kbps)                               │
│ ▼ Top:            (1.01 Mbps)                               │
│ ▼ Total: 7.20 GiB                                           │
│ ▲ 0 Byte/s        (0 bits)                                  │
│ ▲ Top:            (1.31 Mbps)                               │
│ ▲ Total: 1.08 GiB                                           │
│              upload                                           │
└──────────────────────────────────────────────────────────────┘
```
- Download sparkline (Rgb(70,140,255)) + upload sparkline (Rgb(80,200,80))
- Stats: current rate + bits + Top (peak) + Total cumulative

### Panel 8 — Proc Table (bottom-right, 60%×full height)
```
┌─ proc ▼  filter ──────── per-core▼ reverse▼ tree▼ + cpu direct▼ ─┐
│ Pid      Program     Command               Threads User   MemB  Cpu%▼│
│ 2374345  konsole     /usr/bin/konsole            6 klpod  180M   1.6  │
│ 2374824  btop        btop                        2 klpod   12M   0.8  │
│ ...                                                                    │
├────────────────────────────────────────────────────────────────────────┤
│ select ↓↑ info□ terminate□ Kill□ signals□ Nice ──────────── 0/485▼   │
└────────────────────────────────────────────────────────────────────────┘
```
- Columns: **Pid | Program | Command | Threads | User | MemB | Cpu%**
- Sort indicator `▼/▲` trên đúng column
- Bottom action bar
- Proc chiếm toàn bộ height của bottom section

---

## Kiến Trúc Module

```
crates/cli/src/commands/tui/
├── mod.rs            — entry point, event_loop(), draw()
├── state.rs          — AppState, Focus, ProcSort enums
├── theme.rs          — Theme struct + pct_color()
├── layout.rs         — TuiLayout struct + build_layout()
├── utils.rs          — fmt_bytes, fmt_bps, build_bar, build_spark, human_uptime
└── panels/
    ├── mod.rs
    ├── menu_bar.rs   — top menu bar (1 line)
    ├── cpu_graph.rs  — CPU sparkline (top-left-top)
    ├── gpu_graph.rs  — GPU sparkline (top-left-bottom)
    ├── cpu_detail.rs — per-core detail (top-right-top)
    ├── gpu_detail.rs — GPU stats detail (top-right-bottom)
    ├── mem.rs        — memory + swap bars
    ├── disk.rs       — disks + IO sparklines
    ├── net.rs        — network sparklines + stats
    └── proc.rs       — process table
```

---

## Proposed Changes

### [MODIFY] tui.rs → tui/ directory

File `mod.rs` chứa:
- `pub async fn run()` — setup terminal, event_loop
- `event_loop()` — poll events, trigger redraw
- `draw()` — build TuiLayout, dispatch panels

---

### [NEW] state.rs

```rust
pub struct AppState {
    pub cpu: CpuStats,
    pub mem: MemStats,
    pub disks_space: Vec<DiskSpace>,
    pub disks_io: Vec<DiskIo>,
    pub net: Vec<NetInterface>,
    pub procs: Vec<ProcessInfo>,
    pub intel_gpu: Option<IntelGpuStats>,
    pub nvidia_gpus: Vec<NvidiaGpuStats>,
    pub amd_gpus: Vec<AmdGpuStats>,
    // Ring-buffer history (VecDeque = O(1) push/pop vs Vec O(n))
    pub cpu_history: VecDeque<u64>,
    pub gpu_history: VecDeque<u64>,
    pub core_history: Vec<VecDeque<u64>>,
    pub net_rx_history: VecDeque<u64>,
    pub net_tx_history: VecDeque<u64>,
    pub net_rx_peak: f64,
    pub net_tx_peak: f64,
    // UI
    pub focus: Focus,
    pub proc_table: TableState,
    pub proc_sort: ProcSort,
    pub proc_filter: String,
    pub filter_mode: bool,
    pub reverse_sort: bool,
    pub tree_mode: bool,
    pub per_core_mode: bool,
    pub selected_iface: usize,
    pub show_help: bool,
    pub theme: Theme,
    pub refresh_ms: u64,
}

pub enum Focus { CpuGraph, CpuDetail, Mem, Disk, Net, Proc }
pub enum ProcSort { Cpu, Mem, Pid, Name, Threads }
```

---

### [NEW] theme.rs

```rust
pub struct Theme {
    pub border: Color,         // Rgb(60, 60, 80)
    pub border_focused: Color, // Rgb(140, 100, 200)
    pub title: Color,          // Rgb(180, 140, 240)
    pub text: Color,           // Rgb(220, 220, 220)
    pub dim: Color,            // Rgb(100, 100, 120)
    pub good: Color,           // Rgb(80, 200, 80)
    pub warn: Color,           // Rgb(220, 180, 50)
    pub critical: Color,       // Rgb(240, 80, 80)
    pub selected_bg: Color,    // Rgb(40, 40, 70)
    pub cpu_graph: Color,      // Rgb(80, 200, 80)
    pub gpu_graph: Color,      // Rgb(120, 90, 200)
    pub net_rx: Color,         // Rgb(70, 140, 255)
    pub net_tx: Color,         // Rgb(80, 200, 80)
    pub mem_used: Color,       // Rgb(80, 200, 80)
    pub mem_cached: Color,     // Rgb(70, 140, 255)
    pub proc_pid: Color,       // Rgb(180, 140, 240)
    pub proc_cmd: Color,       // Rgb(100, 120, 140)
    pub accent: Color,         // Rgb(140, 100, 200)
}
```

---

### [NEW] layout.rs

```rust
pub struct TuiLayout {
    pub menu_bar: Rect,
    pub cpu_graph: Rect,   // top-left-top
    pub gpu_graph: Rect,   // top-left-bottom
    pub cpu_detail: Rect,  // top-right-top
    pub gpu_detail: Rect,  // top-right-bottom
    pub mem: Rect,         // bottom-left-top-left
    pub disks: Rect,       // bottom-left-top-right
    pub net: Rect,         // bottom-left-bottom
    pub proc: Rect,        // bottom-right full height
}

pub fn build_layout(area: Rect) -> TuiLayout {
    // area
    // ├── menu_bar         [Length(1)]
    // └── main             [Min(0)]
    //     ├── top_row      [Percentage(35)]  Horizontal
    //     │   ├── tl       [Percentage(70)]  Vertical
    //     │   │   ├── cpu_graph  [Percentage(60)]
    //     │   │   └── gpu_graph  [Percentage(40)]
    //     │   └── tr       [Percentage(30)]  Vertical
    //     │       ├── cpu_detail [Percentage(65)]
    //     │       └── gpu_detail [Percentage(35)]
    //     └── bottom_row   [Percentage(65)]  Horizontal
    //         ├── bl       [Percentage(40)]  Vertical
    //         │   ├── bl_top  [Percentage(50)]  Horizontal
    //         │   │   ├── mem   [Percentage(50)]
    //         │   │   └── disks [Percentage(50)]
    //         │   └── net      [Percentage(50)]
    //         └── proc     [Percentage(60)]
}
```

---

## Collector Changes

### [MODIFY] proc.rs — thêm `threads: u32`

Đọc `Threads:` từ `/proc/{pid}/status` trong `read_status()`.

### [MODIFY] net.rs — thêm totals vào `NetInterface`

```rust
pub rx_bytes_total: u64,  // từ /proc/net/dev cột rx_bytes
pub tx_bytes_total: u64,  // từ /proc/net/dev cột tx_bytes
```
Peak (rx_peak, tx_peak) track trong `AppState`, không phải collector.

---

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `q` / `Ctrl-C` | Quit |
| `Tab` | Next panel focus |
| `/` | Enter filter mode |
| `Esc` | Clear filter |
| `↑/↓` / `j/k` | Scroll proc |
| `←/→` | Switch net interface |
| `F6` / `s` | Cycle sort column |
| `r` | Reverse sort |
| `t` | Tree mode |
| `c` | Per-core % mode |
| `Delete` | Kill process (SIGTERM) |
| `?` | Help overlay |

---

## Implementation Order

1. `utils.rs`
2. `theme.rs`
3. `state.rs`
4. `layout.rs`
5. **Collector changes**: proc.rs (threads) + net.rs (totals)
6. `panels/menu_bar.rs`
7. `panels/cpu_graph.rs`
8. `panels/gpu_graph.rs`
9. `panels/cpu_detail.rs`
10. `panels/gpu_detail.rs`
11. `panels/mem.rs`
12. `panels/disk.rs`
13. `panels/net.rs`
14. `panels/proc.rs`
15. `panels/mod.rs` + `mod.rs` — wire, xóa `tui.rs` cũ

---

## Open Questions

> [!IMPORTANT]
> Cần xác nhận trước khi thực thi:

1. **Graph style**: Giữ ratatui `Sparkline` (`▂▃▄▅▆▇█`) hay implement Braille chars (`⡀⡄⡆⡇`) như btop gốc?

2. **GPU panels khi không có GPU**: `gpu_graph` + `gpu_detail` nên **collapse** (cho thêm height vào CPU panels) hay giữ size + hiện placeholder?

3. **Tree mode**: Cần `ppid: u32` trong collector. Có muốn implement không?

4. **Help overlay** (`?` popup): Có làm không?

5. **Mouse click to focus**: Có muốn click để focus panel không?

---

## Verification Plan

```bash
cargo build -p rtop-cli 2>&1
cargo run -p rtop-cli -- tui
```

### Checklist
- [ ] Menu bar clock + interval + filter indicator
- [ ] CPU graph sparkline + uptime
- [ ] GPU graph below CPU
- [ ] CPU detail 2-column cores + load avg
- [ ] GPU detail usage/W/temp + VRAM bar
- [ ] Mem: label+bar per metric, swap section
- [ ] Disk: IO sparkline + Used bar per mount
- [ ] Net: 2 sparklines stacked + stats
- [ ] Proc: 7 columns, bottom action bar, sort indicator
- [ ] All keyboard shortcuts working
