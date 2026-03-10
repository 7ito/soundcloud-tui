use ratatui::{
    Frame,
    layout::{Constraint, Direction, Flex, Layout, Rect},
    widgets::{Clear, Paragraph, Wrap},
};

use crate::app::{AppMode, AppState};
use crate::ui::widgets::pane_block;

pub fn render_app(frame: &mut Frame<'_>, app: &AppState) {
    if app.mode == AppMode::Auth {
        super::auth::render(frame, frame.area(), app);
        return;
    }

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(1),
            Constraint::Length(5),
        ])
        .split(frame.area());

    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(vertical[1]);

    let sidebar = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(middle[0]);

    super::header::render(frame, vertical[0], app);
    super::sidebar::render_library(frame, sidebar[0], app);
    super::sidebar::render_playlists(frame, sidebar[1], app);
    super::content::render(frame, middle[1], app);
    super::playbar::render(frame, vertical[2], app);

    if app.show_help {
        render_help_overlay(frame, app);
    }
}

fn render_help_overlay(frame: &mut Frame<'_>, app: &AppState) {
    let area = centered_rect(frame.area(), 72, 14);
    let text = app.help_overlay_lines().join("\n");
    let popup = Paragraph::new(text)
        .block(pane_block("Help", true))
        .wrap(Wrap { trim: true });

    frame.render_widget(Clear, area);
    frame.render_widget(popup, area);
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(height)])
        .flex(Flex::Center)
        .split(area);
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(width.min(area.width.saturating_sub(2)))])
        .flex(Flex::Center)
        .split(vertical[0]);

    horizontal[0]
}
