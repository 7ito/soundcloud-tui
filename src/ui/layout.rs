use ratatui::Frame;

use crate::app::{AppMode, AppState};
use crate::ui::{cover_art::CoverArtRenderer, geometry};

pub fn render_app(frame: &mut Frame<'_>, app: &AppState, cover_art: &mut CoverArtRenderer) {
    if app.mode == AppMode::Auth {
        super::auth::render(frame, frame.area(), app);
        return;
    }

    if app.visualizer.visible {
        super::visualizer::render(frame, frame.area(), app);
        return;
    }

    let layout = geometry::main_layout(frame.area(), app);

    if layout.header.is_some() {
        super::header::render_search(frame, layout.search, app);
        super::header::render_help(frame, layout.help, app);
        super::header::render_settings(frame, layout.settings, app);
        super::sidebar::render_library(frame, layout.library, app);
        super::sidebar::render_playlists(frame, layout.playlists, app);
    } else {
        super::header::render_search(frame, layout.search, app);
        super::header::render_help(frame, layout.help, app);
        super::header::render_settings(frame, layout.settings, app);
        super::sidebar::render_library(frame, layout.library, app);
        super::sidebar::render_playlists(frame, layout.playlists, app);
    }
    super::content::render(frame, layout.content, app);
    super::playbar::render(frame, layout.playbar, app, cover_art);

    if app.show_welcome {
        super::welcome::render(frame, layout.content, app);
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
