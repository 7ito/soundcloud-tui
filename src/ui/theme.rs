use ratatui::style::Color;

use crate::config::settings::{Settings, parse_color};

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub active: Color,
    pub banner: Color,
    pub hint: Color,
    pub hovered: Color,
    pub selected: Color,
    pub inactive: Color,
    pub text: Color,
    pub error_text: Color,
    pub playbar_background: Color,
    pub playbar_progress: Color,
    pub lyrics_highlight: Color,
    pub highlight_bg: Color,
}

impl Theme {
    pub fn from_settings(settings: &Settings) -> Self {
        let selected = parse_color(&settings.selected_color).unwrap_or(Color::Rgb(255, 95, 31));

        Self {
            active: parse_color(&settings.active_color).unwrap_or(selected),
            banner: parse_color(&settings.banner_color).unwrap_or(selected),
            hint: parse_color(&settings.hint_color).unwrap_or(selected),
            hovered: parse_color(&settings.hovered_color).unwrap_or(selected),
            selected,
            inactive: parse_color(&settings.inactive_color).unwrap_or(Color::DarkGray),
            text: parse_color(&settings.text_color).unwrap_or(Color::Reset),
            error_text: parse_color(&settings.error_text_color)
                .unwrap_or(Color::Rgb(255, 100, 100)),
            playbar_background: parse_color(&settings.playbar_background).unwrap_or(Color::Reset),
            playbar_progress: parse_color(&settings.playbar_progress).unwrap_or(selected),
            lyrics_highlight: parse_color(&settings.lyrics_highlight).unwrap_or(selected),
            highlight_bg: darken(selected, 0.22),
        }
    }
}

fn darken(color: Color, ratio: f32) -> Color {
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(
            ((r as f32) * ratio).round() as u8,
            ((g as f32) * ratio).round() as u8,
            ((b as f32) * ratio).round() as u8,
        ),
        _ => color,
    }
}
