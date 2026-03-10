use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
};

use crate::app::{AppMode, AppState};

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
}
