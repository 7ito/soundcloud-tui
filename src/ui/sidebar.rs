use ratatui::{
    Frame,
    layout::Rect,
    widgets::{List, ListItem, ListState},
};

use crate::{
    app::{AppState, Focus},
    ui::widgets::{HIGHLIGHT_SYMBOL, pane_block, selected_row_style},
};

pub fn render_library(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let items = app
        .library_items
        .iter()
        .map(|item| {
            let label = if item.route == app.route {
                format!("{} *", item.label)
            } else {
                item.label.to_string()
            };
            ListItem::new(label)
        })
        .collect::<Vec<_>>();

    let list = List::new(items)
        .block(pane_block("Library", app.focus == Focus::Library))
        .highlight_style(selected_row_style())
        .highlight_symbol(HIGHLIGHT_SYMBOL);
    let mut state = ListState::default();
    state.select(Some(app.selected_library));

    frame.render_stateful_widget(list, area, &mut state);
}

pub fn render_playlists(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let title = app.playlist_panel_title();
    let items = if let Some(message) = app.playlist_panel_placeholder() {
        vec![ListItem::new(message)]
    } else {
        app.playlists
            .iter()
            .enumerate()
            .map(|(index, playlist)| {
                let label = if app.is_sidebar_playlist_active(index) {
                    format!("{} *", playlist.title)
                } else {
                    playlist.title.clone()
                };
                ListItem::new(label)
            })
            .collect::<Vec<_>>()
    };

    let list = List::new(items)
        .block(pane_block(title.as_str(), app.focus == Focus::Playlists))
        .highlight_style(selected_row_style())
        .highlight_symbol(HIGHLIGHT_SYMBOL);
    let mut state = ListState::default();
    state.select((!app.playlists.is_empty()).then_some(app.selected_playlist));

    frame.render_stateful_widget(list, area, &mut state);
}
