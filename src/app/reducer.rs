use crate::app::{action::Action, route::Focus, state::AppState};

pub fn reduce(state: &mut AppState, action: Action) {
    match action {
        Action::FocusNext => {
            let previous = state.focus;
            let next = state.focus.next();
            if next == Focus::Content {
                state.focus_content_from(previous);
            } else {
                state.set_focus(next);
            }
            state.status = format!("Focused {}.", state.focus.label());
        }
        Action::FocusPrevious => {
            let previous = state.focus;
            let next = state.focus.previous();
            if next == Focus::Content {
                state.focus_content_from(previous);
            } else {
                state.set_focus(next);
            }
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
                "Search editing supports Left/Right, Ctrl+u/k/w/l, and Enter.".to_string()
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
                "Use Space to toggle, </> to seek, +/- for volume, n/p for queue.".to_string();
        }
    }
}

fn select(state: &mut AppState) {
    match state.focus {
        Focus::Search => {
            state.status = "Press Enter while editing to run the current search.".to_string();
        }
        Focus::Library => {
            state.sync_route_from_library();
            state.focus_content_from(Focus::Library);
            state.status = format!("Focused content for {}.", state.route_title());
        }
        Focus::Playlists => {
            state.sync_route_from_playlist();
            state.focus_content_from(Focus::Playlists);
            state.status = format!("Focused content for {}.", state.route_title());
        }
        Focus::Content => state.select_current_content(),
        Focus::Playbar => {
            state.toggle_playback();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Route;

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
        assert_eq!(state.route, Route::Playlist);
    }

    #[test]
    fn selecting_library_focus_moves_into_content() {
        let mut state = AppState::new();

        reduce(&mut state, Action::Select);

        assert_eq!(state.focus, Focus::Content);
        assert_eq!(state.route, Route::Feed);
    }

    #[test]
    fn selecting_playlists_focus_moves_into_content() {
        let mut state = AppState::new();
        state.focus = Focus::Playlists;

        reduce(&mut state, Action::Select);

        assert_eq!(state.focus, Focus::Content);
        assert_eq!(state.route, Route::Playlist);
    }

    #[test]
    fn selecting_search_focus_keeps_current_route() {
        let mut state = AppState::new();
        state.focus = Focus::Search;

        reduce(&mut state, Action::Select);

        assert_eq!(state.route, Route::Feed);
        assert_eq!(
            state.status,
            "Press Enter while editing to run the current search."
        );
    }
}
