use ratatui::{
    style::{Modifier, Style},
    widgets::{Block, BorderType, Borders, Padding},
};

use crate::ui::theme::Theme;

pub const HIGHLIGHT_SYMBOL: &str = "▶ ";

pub fn pane_block(title: &str, is_active: bool) -> Block<'_> {
    let theme = Theme::default();
    let border_style = if is_active {
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.muted)
    };

    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .padding(Padding::horizontal(1))
        .title(title)
        .border_style(border_style)
}

pub fn selected_row_style() -> Style {
    let theme = Theme::default();
    Style::default()
        .fg(theme.accent)
        .bg(theme.highlight_bg)
        .add_modifier(Modifier::BOLD)
}

pub fn header_style() -> Style {
    let theme = Theme::default();
    Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD)
}
