use ratatui::style::Color;

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub accent: Color,
    pub accent_secondary: Color,
    pub accent_tertiary: Color,
    pub muted: Color,
    pub highlight_bg: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            accent: Color::Rgb(255, 95, 31),
            accent_secondary: Color::Rgb(255, 142, 42),
            accent_tertiary: Color::Rgb(255, 196, 103),
            muted: Color::Rgb(136, 118, 108),
            highlight_bg: Color::Rgb(56, 32, 22),
        }
    }
}
