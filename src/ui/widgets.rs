use ratatui::{
    style::{Modifier, Style},
    widgets::{Block, BorderType, Borders, Padding},
};

use crate::app::AppState;

pub const HIGHLIGHT_SYMBOL: &str = "▶ ";

pub fn pane_block<'a>(title: &'a str, is_active: bool, app: &AppState) -> Block<'a> {
    let theme = app.theme();
    let border_style = if is_active {
        emphasis(app, Style::default().fg(theme.active))
    } else {
        Style::default().fg(theme.inactive)
    };

    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .padding(Padding::horizontal(1))
        .title(title)
        .border_style(border_style)
}

pub fn selected_row_style(app: &AppState) -> Style {
    let theme = app.theme();
    emphasis(
        app,
        Style::default().fg(theme.selected).bg(theme.highlight_bg),
    )
}

pub fn header_style(app: &AppState) -> Style {
    let theme = app.theme();
    emphasis(app, Style::default().fg(theme.banner))
}

fn emphasis(app: &AppState, style: Style) -> Style {
    if app.settings().text_emphasis {
        style.add_modifier(Modifier::BOLD)
    } else {
        style
    }
}
