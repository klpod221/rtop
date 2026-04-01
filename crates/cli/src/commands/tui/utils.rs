pub fn fmt_bytes(b: u64) -> String {
    let mut b = b as f64;
    let unit = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut i = 0;
    while b >= 1024.0 && i < unit.len() - 1 {
        b /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{b:.0} {}", unit[i])
    } else {
        format!("{b:.1} {}", unit[i])
    }
}

pub fn fmt_bps(b: f64) -> String {
    let mut b = b;
    let unit = ["Byte/s", "Kbps", "Mbps", "Gbps"];
    let mut i = 0;
    while b >= 1024.0 && i < unit.len() - 1 {
        b /= 1024.0;
        i += 1;
    }
    format!("{b:.2} {}", unit[i])
}

pub fn human_uptime(secs: f64) -> String {
    let secs = secs as u64;
    let d = secs / 86400;
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    if d > 0 {
        format!("{}d {}h:{:02}m", d, h, m)
    } else {
        format!("{}h:{:02}m", h, m)
    }
}

pub fn build_bar(percent: f64, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let p = percent.clamp(0.0, 100.0);
    let filled = (p / 100.0 * width as f64).round() as usize;
    let mut s = String::with_capacity(width);
    for i in 0..width {
        if i < filled {
            s.push('█');
        } else {
            s.push('░');
        }
    }
    s
}

pub fn single_line_braille_spark(data: &std::collections::VecDeque<u64>, width: usize) -> String {
    if width == 0 || data.is_empty() {
        return " ".repeat(width);
    }
    let levels = [' ', '⡀', '⡄', '⡆', '⡇', '⣇', '⣧', '⣷', '⣿'];
    let take_count = width.min(data.len());
    let mut s = String::with_capacity(width);
    let start = data.len().saturating_sub(take_count);
    
    for _ in 0..(width - take_count) {
        s.push(' ');
    }
    
    // We iterate from latest elements backwards?
    // No, data in VecDeque should be older to newer from left to right.
    for i in start..data.len() {
        let val = data[i].clamp(0, 100);
        let idx = (val as f64 / 100.0 * 8.0).round() as usize;
        let idx = idx.clamp(0, 8);
        s.push(levels[idx]);
    }
    s
}

pub fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() > max_len {
        let mut truncated: String = s.chars().take(max_len.saturating_sub(1)).collect();
        truncated.push('…');
        truncated
    } else {
        s.to_string()
    }
}
