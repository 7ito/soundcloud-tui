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

    let wide_search = app.settings().wide_search_bar;
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .spacing(1)
        .constraints(if wide_search {
            vec![
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(app.layout.playbar_height.max(4)),
            ]
        } else {
            vec![
                Constraint::Min(1),
                Constraint::Length(app.layout.playbar_height.max(4)),
            ]
        })
        .split(area);

    let (header_area, body_area, playbar_area) = if wide_search {
        (Some(vertical[0]), vertical[1], vertical[2])
    } else {
        (None, vertical[0], vertical[1])
    };

    let sidebar_width = app.layout.sidebar_width_percent.clamp(14, 40);
    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .spacing(1)
        .constraints([
            Constraint::Percentage(sidebar_width),
            Constraint::Percentage(100 - sidebar_width),
        ])
        .split(body_area);

    let library_height = app.layout.library_height.max(4);
    let sidebar = Layout::default()
        .direction(Direction::Vertical)
        .spacing(1)
        .constraints(if wide_search {
            vec![Constraint::Length(library_height), Constraint::Min(1)]
        } else {
            vec![
                Constraint::Length(3),
                Constraint::Length(library_height),
                Constraint::Min(1),
            ]
        })
        .split(middle[0]);

    if let Some(header_area) = header_area {
        let header = Layout::default()
            .direction(Direction::Horizontal)
            .spacing(1)
            .constraints([
                Constraint::Min(1),
                Constraint::Length(14),
                Constraint::Length(18),
            ])
            .split(header_area);
        super::header::render_search(frame, header[0], app);
        super::header::render_help(frame, header[1], app);
        super::header::render_settings(frame, header[2], app);
        super::sidebar::render_library(frame, sidebar[0], app);
        super::sidebar::render_playlists(frame, sidebar[1], app);
    } else {
        let sidebar_header = Layout::default()
            .direction(Direction::Horizontal)
            .spacing(1)
            .constraints([
                Constraint::Min(1),
                Constraint::Length(10),
                Constraint::Length(14),
            ])
            .split(sidebar[0]);
        super::header::render_search(frame, sidebar_header[0], app);
        super::header::render_help(frame, sidebar_header[1], app);
        super::header::render_settings(frame, sidebar_header[2], app);
        super::sidebar::render_library(frame, sidebar[1], app);
        super::sidebar::render_playlists(frame, sidebar[2], app);
    }
    super::content::render(frame, middle[1], app);
    super::playbar::render(frame, playbar_area, app, cover_art);

    if app.show_welcome {
        super::welcome::render(frame, middle[1], app);
    }

    if app.add_to_playlist_modal.is_some() {
        super::add_to_playlist_modal::render(frame, frame.area(), app);
    }

    if app.queue.overlay_visible {
        super::queue::render(frame, frame.area(), app);
    }

    if app.show_help {
        super::help::render(frame, frame.area(), app);
    }

    if app.show_settings() {
        super::settings::render(frame, frame.area(), app);
    }

    if app.error_modal.is_some() {
        super::error_modal::render(frame, frame.area(), app);
    }
}
