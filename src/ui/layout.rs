use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
};

use crate::app::{AppMode, AppState};
use crate::ui::cover_art::CoverArtRenderer;

pub fn render_app(frame: &mut Frame<'_>, app: &AppState, cover_art: &mut CoverArtRenderer) {
    if app.mode == AppMode::Auth {
        super::auth::render(frame, frame.area(), app);
        return;
    }

    let area = Layout::default()
        .constraints([Constraint::Min(1)])
        .margin(1)
        .split(frame.area())[0];

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .spacing(1)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(app.layout.playbar_height.max(4)),
        ])
        .split(area);

    let sidebar_width = app.layout.sidebar_width_percent.clamp(14, 40);
    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .spacing(1)
        .constraints([
            Constraint::Percentage(sidebar_width),
            Constraint::Percentage(100 - sidebar_width),
        ])
        .split(vertical[0]);

    let library_height = app.layout.library_height.max(4);
    let sidebar = Layout::default()
        .direction(Direction::Vertical)
        .spacing(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(library_height),
            Constraint::Min(1),
        ])
        .split(middle[0]);

    let sidebar_header = Layout::default()
        .direction(Direction::Horizontal)
        .spacing(1)
        .constraints([Constraint::Min(1), Constraint::Length(9)])
        .split(sidebar[0]);

    super::header::render_search(frame, sidebar_header[0], app);
    super::header::render_help(frame, sidebar_header[1], app);
    super::sidebar::render_library(frame, sidebar[1], app);
    super::sidebar::render_playlists(frame, sidebar[2], app);
    super::content::render(frame, middle[1], app);
    super::playbar::render(frame, vertical[1], app, cover_art);

    if app.show_welcome {
        super::welcome::render(frame, middle[1], app);
    }

    if app.error_modal.is_some() {
        super::error_modal::render(frame, frame.area(), app);
    }

    if app.show_help {
        super::help::render(frame, frame.area(), app);
    }
}
