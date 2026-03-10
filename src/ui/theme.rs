use ratatui::style::Color;

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub accent: Color,
    pub muted: Color,
    pub highlight_bg: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            accent: Color::Cyan,
            muted: Color::DarkGray,
            highlight_bg: Color::Rgb(32, 40, 56),
        }
    }
}
