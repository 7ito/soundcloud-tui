use crate::app::{
    action::Action,
    route::{Focus, Route},
    state::AppState,
};

pub fn reduce(state: &mut AppState, action: Action) {
    match action {
        Action::FocusNext => {
            state.focus = state.focus.next();
            state.status = format!("Focused {}.", state.focus.label());
        }
        Action::FocusPrevious => {
            state.focus = state.focus.previous();
            state.status = format!("Focused {}.", state.focus.label());
        }
        Action::MoveUp => move_selection(state, true),
        Action::MoveDown => move_selection(state, false),
        Action::Select => select(state),
        Action::Quit => state.should_quit = true,
    }
}

fn move_selection(state: &mut AppState, up: bool) {
    match state.focus {
        Focus::Search => {
            state.status = if up {
                "Search cursor editing is available with Left/Right, Backspace, Delete, and Enter."
                    .to_string()
            } else {
                "Type a query, then press Enter to search SoundCloud.".to_string()
            };
        }
        Focus::Library => {
            let previous = state.selected_library;
            if up {
                state.selected_library = state.selected_library.saturating_sub(1);
            } else if state.selected_library + 1 < state.library_items.len() {
                state.selected_library += 1;
            }

            if previous != state.selected_library {
                state.sync_route_from_library();
            }
        }
        Focus::Playlists => {
            let previous = state.selected_playlist;
            if up {
                state.selected_playlist = state.selected_playlist.saturating_sub(1);
            } else if state.selected_playlist + 1 < state.playlists.len() {
                state.selected_playlist += 1;
            } else if state.maybe_queue_more_playlists() {
                return;
            }

            if previous != state.selected_playlist {
                state.sync_route_from_playlist();
            }
        }
        Focus::Content => {
            let previous = state.selected_content;
            if up {
                state.selected_content = state.selected_content.saturating_sub(1);
            } else if state.selected_content + 1 < state.current_content_len() {
                state.selected_content += 1;
            } else if state.maybe_queue_current_route_next_page() {
                return;
            }

            if previous != state.selected_content {
                if let Some(label) = state.current_selection_label() {
                    state.status = format!("Highlighted {}.", label);
                }
            }
        }
        Focus::Playbar => {
            state.status =
                "Use Space to toggle, Left/Right to seek, +/- for volume, n/p for queue."
                    .to_string();
        }
    }
}

fn select(state: &mut AppState) {
    match state.focus {
        Focus::Search => state.set_route(Route::Search),
        Focus::Library => state.sync_route_from_library(),
        Focus::Playlists => state.sync_route_from_playlist(),
        Focus::Content => state.select_current_content(),
        Focus::Playbar => {
            state.toggle_playback();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_cycles_forward_and_backward() {
        let mut state = AppState::new();

        reduce(&mut state, Action::FocusNext);
        assert_eq!(state.focus, Focus::Playlists);

        reduce(&mut state, Action::FocusPrevious);
        assert_eq!(state.focus, Focus::Library);
    }

    #[test]
    fn moving_library_selection_updates_route() {
        let mut state = AppState::new();

        reduce(&mut state, Action::MoveDown);

        assert_eq!(state.selected_library, 1);
        assert_eq!(state.route, Route::LikedSongs);
        assert_eq!(state.selected_content, 0);
    }

    #[test]
    fn moving_playlist_selection_updates_route() {
        let mut state = AppState::new();
        state.focus = Focus::Playlists;

        reduce(&mut state, Action::MoveDown);

        assert_eq!(state.selected_playlist, 1);
        assert_eq!(state.route, Route::Playlist(1));
    }

    #[test]
    fn selecting_search_focus_opens_search_route() {
        let mut state = AppState::new();
        state.focus = Focus::Search;

        reduce(&mut state, Action::Select);

        assert_eq!(state.route, Route::Search);
        assert_eq!(state.selected_content, 0);
    }
}
