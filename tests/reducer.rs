use soundcloud_tui::app::{Action, AppState, Focus, Route};

#[test]
fn library_navigation_changes_route() {
    let mut app = AppState::new();

    app.apply(Action::MoveDown);
    app.apply(Action::MoveDown);

    assert_eq!(app.route, Route::RecentlyPlayed);
    assert_eq!(app.selected_library, 2);
}

#[test]
fn pane_focus_cycles_across_main_panes_only() {
    let mut app = AppState::new();

    app.apply(Action::FocusNext);
    app.apply(Action::FocusNext);
    app.apply(Action::FocusNext);
    app.apply(Action::FocusNext);

    assert_eq!(app.focus, Focus::Library);
}
