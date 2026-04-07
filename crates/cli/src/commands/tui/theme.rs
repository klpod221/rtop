use ratatui::style::Color;

#[derive(Clone, Copy)]
pub struct Theme {
    pub border: Color,
    pub border_focused: Color,
    pub title: Color,
    pub text: Color,
    pub dim: Color,
    pub good: Color,
    pub warn: Color,
    pub critical: Color,
    pub selected_bg: Color,
    pub net_rx: Color,
    pub net_tx: Color,
    pub mem_used: Color,
    pub mem_cached: Color,
    pub accent: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Theme::default_dark()
    }
}

impl Theme {
    pub fn default_dark() -> Self {
        Self {
            border: Color::Rgb(60, 60, 80),
            border_focused: Color::Rgb(140, 100, 200),
            title: Color::Rgb(180, 140, 240),
            text: Color::Rgb(220, 220, 220),
            dim: Color::Rgb(100, 100, 120),
            good: Color::Rgb(80, 200, 80),
            warn: Color::Rgb(220, 180, 50),
            critical: Color::Rgb(240, 80, 80),
            selected_bg: Color::Rgb(40, 40, 70),
            net_rx: Color::Rgb(70, 140, 255),
            net_tx: Color::Rgb(80, 200, 80),
            mem_used: Color::Rgb(80, 200, 80),
            mem_cached: Color::Rgb(70, 140, 255),
            accent: Color::Rgb(140, 100, 200),
        }
    }

    pub fn pct_color(&self, pct: f64) -> Color {
        if pct < 50.0 {
            self.good
        } else if pct < 85.0 {
            self.warn
        } else {
            self.critical
        }
    }
}
