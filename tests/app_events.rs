use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use soundcloud_tui::{
    app::{AppCommand, AppEvent, AppMode, AppState, AuthFocus, AuthStep, Focus, Route},
    config::{
        credentials::Credentials,
        history::{RecentlyPlayedEntry, RecentlyPlayedStore},
        settings::{Settings, StartupBehavior},
        tokens::TokenStore,
    },
    player::command::PlayerCommand,
    player::event::PlayerEvent,
    soundcloud::{
        auth::{self, AuthSession, AuthorizedSession},
        models::{PlaylistSummary, SearchResults, TrackSummary, UserSummary},
        paging::Page,
    },
};

#[test]
fn tick_event_advances_loading_state() {
    let mut app = AppState::new();

    assert!(app.loading.is_some());

    app.dispatch_event(AppEvent::Tick);
    app.dispatch_event(AppEvent::Tick);

    assert!(app.loading.is_none());
    assert_eq!(app.tick_count, 2);
}

#[test]
fn resize_event_updates_viewport() {
    let mut app = AppState::new();

    app.dispatch_event(AppEvent::Resize {
        width: 120,
        height: 40,
    });

    assert_eq!(app.viewport.width, 120);
    assert_eq!(app.viewport.height, 40);
}

#[test]
fn action_event_routes_through_dispatch() {
    let mut app = AppState::new();

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )));

    assert_eq!(app.route, Route::Feed);
}

#[test]
fn save_and_continue_queues_credentials_persist_first() {
    let credentials = Credentials {
        client_id: "client-id".to_string(),
        client_secret: "client-secret".to_string(),
        redirect_uri: "http://127.0.0.1:8974/callback".to_string(),
    };
    let mut app = AppState::new_onboarding(credentials.clone());
    app.auth.focus = AuthFocus::SaveAndContinue;

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )));

    match app.take_pending_command() {
        Some(AppCommand::SaveCredentials(request)) => {
            assert_eq!(request.credentials.client_id, credentials.client_id);
            assert_eq!(request.credentials.redirect_uri, credentials.redirect_uri);
            assert!(request.authorize_url.contains("client_id=client-id"));
        }
        other => panic!("expected SaveCredentials command, got {other:?}"),
    }

    assert!(app.take_pending_command().is_none());
}

#[test]
fn credentials_saved_event_starts_browser_flow() {
    let credentials = Credentials {
        client_id: "client-id".to_string(),
        client_secret: "client-secret".to_string(),
        redirect_uri: "http://127.0.0.1:8974/callback".to_string(),
    };
    let request = auth::prepare_authorization(credentials).expect("authorization request");
    let expected_url = request.authorize_url.clone();
    let mut app = AppState::new_onboarding(request.credentials.clone());

    app.dispatch_event(AppEvent::CredentialsSaved(request));

    assert_eq!(app.auth.step, AuthStep::WaitingForBrowser);
    assert_eq!(
        app.status,
        "Saved credentials locally. Authorize the app in your browser."
    );

    match app.take_pending_command() {
        Some(AppCommand::OpenUrl(url)) => assert_eq!(url, expected_url),
        other => panic!("expected OpenUrl command, got {other:?}"),
    }

    assert!(matches!(
        app.take_pending_command(),
        Some(AppCommand::WaitForOAuthCallback(_))
    ));
}

#[test]
fn player_position_updates_now_playing_progress() {
    let mut app = AppState::new();
    let track = dummy_track("soundcloud:tracks:1", "First Track");
    app.now_playing.track = Some(track.clone());
    app.now_playing.title = track.title;
    app.player.duration_seconds = Some(200.0);

    app.dispatch_event(AppEvent::Player(PlayerEvent::PositionChanged {
        seconds: 50.0,
    }));

    assert_eq!(app.now_playing.elapsed_label, "0:50");
    assert_eq!(app.now_playing.progress_ratio, 0.25);
}

#[test]
fn track_end_advances_to_next_queue_item() {
    let mut app = AppState::new();
    let first = dummy_track("soundcloud:tracks:1", "First Track");
    let second = dummy_track("soundcloud:tracks:2", "Second Track");
    app.session = Some(dummy_session());
    seed_liked_tracks(&mut app, vec![first.clone(), second.clone()]);

    app.select_current_content();
    drain_pending_commands(&mut app);

    app.dispatch_event(AppEvent::Player(PlayerEvent::TrackEnded));

    match app.take_pending_command() {
        Some(AppCommand::PlayTrack { track, .. }) => assert_eq!(track.title, second.title),
        other => panic!("expected PlayTrack command, got {other:?}"),
    }

    assert_eq!(app.now_playing.title, second.title);
}

#[test]
fn uppercase_q_opens_queue_overlay() {
    let mut app = AppState::new();
    let track = dummy_track("soundcloud:tracks:30", "Queued Track");
    app.session = Some(dummy_session());
    seed_liked_tracks(&mut app, vec![track.clone()]);

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('z'),
        KeyModifiers::NONE,
    )));
    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('Q'),
        KeyModifiers::SHIFT,
    )));

    assert!(app.queue.overlay_visible);
    assert_eq!(app.queue_overlay_rows().len(), 1);
    assert_eq!(app.queue_overlay_rows()[0].columns[0], track.title);
}

#[test]
fn queue_shortcut_appends_without_interrupting_playback() {
    let mut app = AppState::new();
    let first = dummy_track("soundcloud:tracks:31", "Playing Track");
    let second = dummy_track("soundcloud:tracks:32", "Queued Track");
    app.session = Some(dummy_session());
    seed_liked_tracks(&mut app, vec![first.clone(), second.clone()]);

    app.select_current_content();
    drain_pending_commands(&mut app);
    app.selected_content = 1;

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('z'),
        KeyModifiers::NONE,
    )));

    assert_eq!(app.now_playing.title, first.title);
    assert!(app.take_pending_command().is_none());
    assert_eq!(app.queue_overlay_rows().len(), 1);
    assert_eq!(app.queue_overlay_rows()[0].columns[0], second.title);
}

#[test]
fn queue_overlay_enter_starts_selected_track() {
    let mut app = AppState::new();
    let first = dummy_track("soundcloud:tracks:33", "First Queue Track");
    let second = dummy_track("soundcloud:tracks:34", "Second Queue Track");
    app.session = Some(dummy_session());
    seed_liked_tracks(&mut app, vec![first.clone(), second.clone()]);

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('z'),
        KeyModifiers::NONE,
    )));
    app.selected_content = 1;
    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('z'),
        KeyModifiers::NONE,
    )));
    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('Q'),
        KeyModifiers::SHIFT,
    )));
    app.queue.selected = 1;

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )));

    match app.take_pending_command() {
        Some(AppCommand::PlayTrack { track, .. }) => assert_eq!(track.title, second.title),
        other => panic!("expected PlayTrack command, got {other:?}"),
    }

    assert!(app.queue.overlay_visible);
    assert_eq!(app.now_playing.title, second.title);
    assert_eq!(app.queue_overlay_rows()[0].columns[0], second.title);
}

#[test]
fn queue_overlay_d_removes_selected_track() {
    let mut app = AppState::new();
    let first = dummy_track("soundcloud:tracks:35", "Keep Me");
    let second = dummy_track("soundcloud:tracks:36", "Drop Me");
    app.session = Some(dummy_session());
    seed_liked_tracks(&mut app, vec![first.clone(), second.clone()]);

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('z'),
        KeyModifiers::NONE,
    )));
    app.selected_content = 1;
    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('z'),
        KeyModifiers::NONE,
    )));
    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('Q'),
        KeyModifiers::SHIFT,
    )));
    app.queue.selected = 1;

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('d'),
        KeyModifiers::NONE,
    )));

    assert_eq!(app.queue_overlay_rows().len(), 1);
    assert_eq!(app.queue_overlay_rows()[0].columns[0], first.title);
}

#[test]
fn manual_selection_preserves_queue_then_resumes_source_tail() {
    let mut app = AppState::new();
    let queue_first = dummy_track("soundcloud:tracks:37", "Queue First");
    let queue_second = dummy_track("soundcloud:tracks:38", "Queue Second");
    let source_first = dummy_track("soundcloud:tracks:39", "Source First");
    let source_second = dummy_track("soundcloud:tracks:40", "Source Second");
    let source_third = dummy_track("soundcloud:tracks:41", "Source Third");
    app.session = Some(dummy_session());

    seed_liked_tracks(&mut app, vec![queue_first.clone(), queue_second.clone()]);
    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('z'),
        KeyModifiers::NONE,
    )));
    app.selected_content = 1;
    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('z'),
        KeyModifiers::NONE,
    )));

    seed_search_tracks(
        &mut app,
        "manual",
        vec![source_first, source_second.clone(), source_third.clone()],
    );
    app.selected_content = 1;

    app.select_current_content();
    drain_pending_commands(&mut app);

    app.dispatch_event(AppEvent::Player(PlayerEvent::TrackEnded));
    match app.take_pending_command() {
        Some(AppCommand::PlayTrack { track, .. }) => assert_eq!(track.title, queue_first.title),
        other => panic!("expected PlayTrack command, got {other:?}"),
    }
    assert_eq!(app.now_playing.context, "Queue");

    app.dispatch_event(AppEvent::Player(PlayerEvent::TrackEnded));
    match app.take_pending_command() {
        Some(AppCommand::PlayTrack { track, .. }) => assert_eq!(track.title, queue_second.title),
        other => panic!("expected PlayTrack command, got {other:?}"),
    }

    app.dispatch_event(AppEvent::Player(PlayerEvent::TrackEnded));
    match app.take_pending_command() {
        Some(AppCommand::PlayTrack { track, .. }) => assert_eq!(track.title, source_third.title),
        other => panic!("expected PlayTrack command, got {other:?}"),
    }
    assert_eq!(app.now_playing.context, "Search: manual");
}

#[test]
fn auth_complete_shows_help_and_persists_dismissal() {
    let mut app = AppState::new_onboarding_with_persistence(
        Credentials::default(),
        Settings {
            theme: "default".to_string(),
            show_help_on_startup: true,
            ..Settings::default()
        },
        RecentlyPlayedStore::default(),
    );

    app.dispatch_event(AppEvent::AuthCompleted(Ok(dummy_session())));

    assert_eq!(app.mode, AppMode::Main);
    assert!(app.show_help);

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )));

    assert!(!app.show_help);

    let mut saved = None;
    while let Some(command) = app.take_pending_command() {
        if let AppCommand::SaveSettings(settings) = command {
            saved = Some(settings);
        }
    }

    let saved = saved.expect("expected SaveSettings command");
    assert!(!saved.show_help_on_startup);
}

#[test]
fn playback_started_populates_recently_played_and_queues_save() {
    let mut app = AppState::new();
    let track = dummy_track("soundcloud:tracks:1", "First Track");
    app.session = Some(dummy_session());
    app.now_playing.track = Some(track.clone());
    app.now_playing.title = track.title.clone();
    app.now_playing.artist = track.artist.clone();
    app.now_playing.context = "Liked Songs".to_string();

    app.dispatch_event(AppEvent::Player(PlayerEvent::PlaybackStarted));
    app.set_route(Route::RecentlyPlayed);

    let view = app.current_content();
    assert_eq!(view.rows[0].columns[0], track.title);
    assert_eq!(view.rows[0].columns[2], "Liked Songs");

    match app.take_pending_command() {
        Some(AppCommand::SaveHistory(history)) => {
            assert_eq!(history.entries.len(), 1);
            assert_eq!(history.entries[0].track.urn, "soundcloud:tracks:1");
        }
        other => panic!("expected SaveHistory command, got {other:?}"),
    }
}

#[test]
fn search_result_shortcuts_switch_between_tables() {
    let mut app = AppState::new();
    let playlist = dummy_playlist("soundcloud:playlists:1", "Night Drive");
    app.session = Some(dummy_session());
    app.search_query = "night".to_string();
    app.search_cursor = 5;
    app.set_route(Route::Search);
    while app.take_pending_command().is_some() {}

    app.dispatch_event(AppEvent::SearchLoaded {
        session: dummy_session(),
        query: "night".to_string(),
        results: SearchResults {
            tracks: Page {
                items: vec![dummy_track("soundcloud:tracks:9", "Night Track")],
                next_href: None,
            },
            playlists: Page {
                items: vec![playlist.clone()],
                next_href: None,
            },
            users: Page {
                items: vec![dummy_user()],
                next_href: None,
            },
        },
    });

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('2'),
        KeyModifiers::NONE,
    )));
    assert_eq!(app.current_content().columns[0], "Playlist");

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('3'),
        KeyModifiers::NONE,
    )));
    assert_eq!(app.current_content().columns[0], "Creator");

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('1'),
        KeyModifiers::NONE,
    )));
    assert_eq!(app.current_content().columns[0], "Title");
}

#[test]
fn slash_enters_search_and_enter_submits_typed_query() {
    let mut app = AppState::new();

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('/'),
        KeyModifiers::NONE,
    )));
    assert_eq!(app.focus, Focus::Search);

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('n'),
        KeyModifiers::NONE,
    )));
    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('i'),
        KeyModifiers::NONE,
    )));
    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('g'),
        KeyModifiers::NONE,
    )));
    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('h'),
        KeyModifiers::NONE,
    )));
    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('t'),
        KeyModifiers::NONE,
    )));

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )));

    assert_eq!(app.focus, Focus::Content);
    assert_eq!(app.route, Route::Search);
    assert_eq!(app.search_query, "night");
}

#[test]
fn enter_in_library_moves_focus_into_content() {
    let mut app = AppState::new();

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )));

    assert_eq!(app.focus, Focus::Content);
    assert_eq!(app.route, Route::Feed);
}

#[test]
fn enter_in_playlists_moves_focus_into_content() {
    let mut app = AppState::new();
    app.focus = Focus::Playlists;

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )));

    assert_eq!(app.focus, Focus::Content);
    assert_eq!(app.route, Route::Playlist);
}

#[test]
fn selecting_content_without_session_does_not_replace_now_playing() {
    let mut app = AppState::new();
    app.focus = Focus::Content;

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )));

    assert_eq!(app.now_playing.track, None);
    assert_eq!(app.now_playing.title, "Nothing playing");
    assert!(
        app.status
            .contains("Playback is unavailable until SoundCloud authentication is complete")
    );
}

#[test]
fn ctrl_r_cycles_repeat_and_ctrl_s_toggles_shuffle() {
    let mut app = AppState::new();

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('r'),
        KeyModifiers::CONTROL,
    )));
    assert_eq!(app.player.repeat_mode.label(), "Track");

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('s'),
        KeyModifiers::CONTROL,
    )));
    assert!(app.player.shuffle_enabled);
}

#[test]
fn seek_and_volume_shortcuts_use_requested_keys() {
    let mut app = AppState::new();
    let track = dummy_track("soundcloud:tracks:10", "Shortcut Track");
    app.now_playing.track = Some(track.clone());
    app.now_playing.title = track.title;

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('>'),
        KeyModifiers::SHIFT,
    )));
    match app.take_pending_command() {
        Some(AppCommand::ControlPlayback(PlayerCommand::SeekRelative { seconds })) => {
            assert_eq!(seconds, 5.0)
        }
        other => panic!("expected forward seek command, got {other:?}"),
    }

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('+'),
        KeyModifiers::SHIFT,
    )));
    match app.take_pending_command() {
        Some(AppCommand::ControlPlayback(PlayerCommand::SetVolume { percent })) => {
            assert_eq!(percent, 60.0)
        }
        other => panic!("expected volume command, got {other:?}"),
    }
}

#[test]
fn search_input_shortcuts_edit_query() {
    let mut app = AppState::new();
    app.focus = Focus::Search;
    app.search_query = "night drive".to_string();
    app.search_cursor = app.search_query.len();

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('i'),
        KeyModifiers::CONTROL,
    )));
    assert_eq!(app.search_cursor, 0);

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('o'),
        KeyModifiers::CONTROL,
    )));
    assert_eq!(app.search_cursor, app.search_query.len());

    app.search_cursor = 5;
    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('k'),
        KeyModifiers::CONTROL,
    )));
    assert_eq!(app.search_query, "night");

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char(' '),
        KeyModifiers::NONE,
    )));
    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('m'),
        KeyModifiers::NONE,
    )));
    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('i'),
        KeyModifiers::NONE,
    )));
    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('x'),
        KeyModifiers::NONE,
    )));

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('w'),
        KeyModifiers::CONTROL,
    )));
    assert_eq!(app.search_query, "night ");

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('u'),
        KeyModifiers::CONTROL,
    )));
    assert_eq!(app.search_query, "");
    assert_eq!(app.search_cursor, 0);

    app.search_query = "reset me".to_string();
    app.search_cursor = app.search_query.len();
    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('l'),
        KeyModifiers::CONTROL,
    )));
    assert_eq!(app.search_query, "");
}

#[test]
fn search_input_keeps_plain_w_for_typing() {
    let mut app = AppState::new();
    app.focus = Focus::Search;

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('w'),
        KeyModifiers::NONE,
    )));

    assert_eq!(app.search_query, "w");
    assert!(app.add_to_playlist_modal.is_none());
}

#[test]
fn esc_in_content_returns_to_previous_pane() {
    let mut app = AppState::new();
    app.focus = Focus::Playlists;

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )));
    assert_eq!(app.focus, Focus::Content);

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Esc,
        KeyModifiers::NONE,
    )));
    assert_eq!(app.focus, Focus::Playlists);
}

#[test]
fn help_menu_scrolls_with_ctrl_d_and_ctrl_u() {
    let mut app = AppState::new();
    app.dispatch_event(AppEvent::Resize {
        width: 120,
        height: 16,
    });

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('?'),
        KeyModifiers::SHIFT,
    )));
    assert!(app.show_help);

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('d'),
        KeyModifiers::CONTROL,
    )));
    assert!(app.help_scroll > 0);

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('u'),
        KeyModifiers::CONTROL,
    )));
    assert_eq!(app.help_scroll, 0);
}

#[test]
fn f1_opens_and_closes_help_menu() {
    let mut app = AppState::new();

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::F(1),
        KeyModifiers::NONE,
    )));
    assert!(app.show_help);

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::F(1),
        KeyModifiers::NONE,
    )));
    assert!(!app.show_help);
}

#[test]
fn open_settings_shortcut_and_save_persists_behavior_changes() {
    let mut app = AppState::new();

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char(','),
        KeyModifiers::ALT,
    )));
    assert!(app.show_settings());

    for _ in 0..5 {
        app.dispatch_event(AppEvent::Key(KeyEvent::new(
            KeyCode::Char('j'),
            KeyModifiers::NONE,
        )));
    }

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )));
    assert!(
        app.settings_menu
            .as_ref()
            .expect("settings open")
            .draft
            .wide_search_bar
    );

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('s'),
        KeyModifiers::ALT,
    )));

    assert!(app.settings().wide_search_bar);
    match app.take_pending_command() {
        Some(AppCommand::SaveSettings(settings)) => assert!(settings.wide_search_bar),
        other => panic!("expected SaveSettings command, got {other:?}"),
    }
}

#[test]
fn startup_behavior_play_queues_recent_track_on_auth_complete() {
    let track = dummy_track("soundcloud:tracks:500", "Startup Track");
    let history = RecentlyPlayedStore {
        entries: vec![RecentlyPlayedEntry {
            track: track.clone(),
            context: "Liked Songs".to_string(),
            played_at_epoch: 0,
        }],
    };
    let mut app = AppState::new_onboarding_with_persistence(
        Credentials::default(),
        Settings {
            show_help_on_startup: false,
            startup_behavior: StartupBehavior::Play,
            ..Settings::default()
        },
        history,
    );

    app.dispatch_event(AppEvent::AuthCompleted(Ok(dummy_session())));

    match app.take_pending_command() {
        Some(AppCommand::PlayTrack { track: queued, .. }) => assert_eq!(queued.title, track.title),
        other => panic!("expected PlayTrack command, got {other:?}"),
    }
}

#[test]
fn copy_shortcut_queues_clipboard_command() {
    let mut app = AppState::new();
    let track = dummy_track("soundcloud:tracks:11", "Share Me");
    app.now_playing.track = Some(track.clone());
    app.now_playing.title = track.title.clone();

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('c'),
        KeyModifiers::NONE,
    )));

    match app.take_pending_command() {
        Some(AppCommand::CopyText { text, label }) => {
            assert_eq!(text, "https://soundcloud.com/tester/Share Me");
            assert_eq!(label, track.title);
        }
        other => panic!("expected CopyText command, got {other:?}"),
    }
}

#[test]
fn like_shortcut_queues_track_like_for_selected_track() {
    let mut app = AppState::new();
    let track = dummy_track("soundcloud:tracks:12", "Like Me");
    app.session = Some(dummy_session());

    app.dispatch_event(AppEvent::LikedSongsLoaded {
        session: dummy_session(),
        page: Page {
            items: vec![track.clone()],
            next_href: None,
        },
        append: false,
    });
    app.set_route(Route::LikedSongs);
    while app.take_pending_command().is_some() {}
    app.focus = Focus::Content;

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('l'),
        KeyModifiers::NONE,
    )));

    match app.take_pending_command() {
        Some(AppCommand::LikeTrack { track: queued, .. }) => assert_eq!(queued.title, track.title),
        other => panic!("expected LikeTrack command, got {other:?}"),
    }
}

#[test]
fn playlist_shortcut_opens_modal_and_enter_queues_add() {
    let mut app = AppState::new();
    let track = dummy_track("soundcloud:tracks:13", "Queue Me");
    let playlist = dummy_playlist("soundcloud:playlists:3", "Road Trip");
    app.session = Some(dummy_session());

    app.dispatch_event(AppEvent::PlaylistsLoaded {
        session: dummy_session(),
        page: Page {
            items: vec![playlist.clone()],
            next_href: None,
        },
        append: false,
    });
    app.dispatch_event(AppEvent::LikedSongsLoaded {
        session: dummy_session(),
        page: Page {
            items: vec![track.clone()],
            next_href: None,
        },
        append: false,
    });
    app.set_route(Route::LikedSongs);
    while app.take_pending_command().is_some() {}
    app.focus = Focus::Content;

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('w'),
        KeyModifiers::NONE,
    )));
    assert!(app.add_to_playlist_modal.is_some());

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )));

    match app.take_pending_command() {
        Some(AppCommand::AddTrackToPlaylist {
            track: queued_track,
            playlist: queued_playlist,
            ..
        }) => {
            assert_eq!(queued_track.title, track.title);
            assert_eq!(queued_playlist.title, playlist.title);
        }
        other => panic!("expected AddTrackToPlaylist command, got {other:?}"),
    }
    assert!(app.add_to_playlist_modal.is_none());
}

#[test]
fn playlist_modal_supports_navigation_jumps_and_cancel() {
    let mut app = AppState::new();
    let track = dummy_track("soundcloud:tracks:14", "Navigate Me");
    let playlists = vec![
        dummy_playlist("soundcloud:playlists:10", "One"),
        dummy_playlist("soundcloud:playlists:11", "Two"),
        dummy_playlist("soundcloud:playlists:12", "Three"),
    ];
    app.session = Some(dummy_session());

    app.dispatch_event(AppEvent::PlaylistsLoaded {
        session: dummy_session(),
        page: Page {
            items: playlists,
            next_href: None,
        },
        append: false,
    });
    app.dispatch_event(AppEvent::LikedSongsLoaded {
        session: dummy_session(),
        page: Page {
            items: vec![track],
            next_href: None,
        },
        append: false,
    });
    app.set_route(Route::LikedSongs);
    while app.take_pending_command().is_some() {}
    app.focus = Focus::Content;

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('w'),
        KeyModifiers::NONE,
    )));
    assert_eq!(
        app.add_to_playlist_modal
            .as_ref()
            .expect("modal should open")
            .selected_playlist,
        0
    );

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Down,
        KeyModifiers::NONE,
    )));
    assert_eq!(
        app.add_to_playlist_modal
            .as_ref()
            .expect("modal should stay open")
            .selected_playlist,
        1
    );

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('L'),
        KeyModifiers::SHIFT,
    )));
    assert_eq!(
        app.add_to_playlist_modal
            .as_ref()
            .expect("modal should stay open")
            .selected_playlist,
        2
    );

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Esc,
        KeyModifiers::NONE,
    )));
    assert!(app.add_to_playlist_modal.is_none());
}

#[test]
fn lowercase_shortcuts_do_not_run_outside_content() {
    let mut app = AppState::new();
    let selected_track = dummy_track("soundcloud:tracks:18", "Selected Track");
    let now_playing_track = dummy_track("soundcloud:tracks:19", "Now Playing Track");
    let playlist = dummy_playlist("soundcloud:playlists:18", "Focus Test");
    app.session = Some(dummy_session());
    app.now_playing.track = Some(now_playing_track.clone());
    app.now_playing.title = now_playing_track.title.clone();

    app.dispatch_event(AppEvent::PlaylistsLoaded {
        session: dummy_session(),
        page: Page {
            items: vec![playlist],
            next_href: None,
        },
        append: false,
    });
    app.dispatch_event(AppEvent::LikedSongsLoaded {
        session: dummy_session(),
        page: Page {
            items: vec![selected_track],
            next_href: None,
        },
        append: false,
    });
    app.set_route(Route::LikedSongs);
    while app.take_pending_command().is_some() {}

    for focus in [Focus::Library, Focus::Playlists, Focus::Playbar] {
        app.focus = focus;

        app.dispatch_event(AppEvent::Key(KeyEvent::new(
            KeyCode::Char('l'),
            KeyModifiers::NONE,
        )));
        app.dispatch_event(AppEvent::Key(KeyEvent::new(
            KeyCode::Char('z'),
            KeyModifiers::NONE,
        )));
        app.dispatch_event(AppEvent::Key(KeyEvent::new(
            KeyCode::Char('w'),
            KeyModifiers::NONE,
        )));

        assert!(app.take_pending_command().is_none());
        assert!(app.add_to_playlist_modal.is_none());
        assert!(app.queue_overlay_rows().is_empty());
    }
}

#[test]
fn uppercase_like_shortcut_targets_now_playing_track() {
    let mut app = AppState::new();
    let selected_track = dummy_track("soundcloud:tracks:20", "Selected Track");
    let now_playing_track = dummy_track("soundcloud:tracks:21", "Current Track");
    app.session = Some(dummy_session());
    app.now_playing.track = Some(now_playing_track.clone());
    app.now_playing.title = now_playing_track.title.clone();

    app.dispatch_event(AppEvent::LikedSongsLoaded {
        session: dummy_session(),
        page: Page {
            items: vec![selected_track],
            next_href: None,
        },
        append: false,
    });
    app.set_route(Route::LikedSongs);
    while app.take_pending_command().is_some() {}
    app.focus = Focus::Library;

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('L'),
        KeyModifiers::SHIFT,
    )));

    match app.take_pending_command() {
        Some(AppCommand::LikeTrack { track: queued, .. }) => {
            assert_eq!(queued.title, now_playing_track.title)
        }
        other => panic!("expected LikeTrack command, got {other:?}"),
    }
}

#[test]
fn uppercase_playlist_shortcut_uses_now_playing_track() {
    let mut app = AppState::new();
    let selected_track = dummy_track("soundcloud:tracks:22", "Selected Track");
    let now_playing_track = dummy_track("soundcloud:tracks:23", "Current Track");
    let playlist = dummy_playlist("soundcloud:playlists:22", "Night Drive");
    app.session = Some(dummy_session());
    app.now_playing.track = Some(now_playing_track.clone());
    app.now_playing.title = now_playing_track.title.clone();

    app.dispatch_event(AppEvent::PlaylistsLoaded {
        session: dummy_session(),
        page: Page {
            items: vec![playlist.clone()],
            next_href: None,
        },
        append: false,
    });
    app.dispatch_event(AppEvent::LikedSongsLoaded {
        session: dummy_session(),
        page: Page {
            items: vec![selected_track],
            next_href: None,
        },
        append: false,
    });
    app.set_route(Route::LikedSongs);
    while app.take_pending_command().is_some() {}
    app.focus = Focus::Playbar;

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('W'),
        KeyModifiers::SHIFT,
    )));

    let modal = app
        .add_to_playlist_modal
        .as_ref()
        .expect("modal should open for now playing track");
    assert_eq!(modal.track.title, now_playing_track.title);

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )));

    match app.take_pending_command() {
        Some(AppCommand::AddTrackToPlaylist {
            track: queued_track,
            playlist: queued_playlist,
            ..
        }) => {
            assert_eq!(queued_track.title, now_playing_track.title);
            assert_eq!(queued_playlist.title, playlist.title);
        }
        other => panic!("expected AddTrackToPlaylist command, got {other:?}"),
    }
}

#[test]
fn clipboard_copy_failure_opens_dismissible_error_modal() {
    let mut app = AppState::new();

    app.dispatch_event(AppEvent::ClipboardCopyFailed {
        label: "Share Me".to_string(),
        error: "clipboard unavailable".to_string(),
    });

    let error_modal = app.error_modal.as_ref().expect("expected error modal");
    assert_eq!(error_modal.title, "Could not copy share URL");
    assert!(error_modal.message.contains("clipboard unavailable"));

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Esc,
        KeyModifiers::NONE,
    )));

    assert!(app.error_modal.is_none());
}

#[test]
fn route_load_failures_open_error_modal() {
    let mut app = AppState::new();

    app.dispatch_event(AppEvent::FeedFailed("network timeout".to_string()));

    let error_modal = app.error_modal.as_ref().expect("expected error modal");
    assert_eq!(error_modal.title, "Could not load feed");
    assert_eq!(app.status, "Could not load feed");
}

#[test]
fn clipboard_success_shows_temporary_toast() {
    let mut app = AppState::new();

    app.dispatch_event(AppEvent::ClipboardCopied {
        label: "Share Me".to_string(),
    });

    let toast = app.toast.as_ref().expect("expected toast");
    assert_eq!(toast.message, "Copied URL to clipboard");

    for _ in 0..11 {
        app.dispatch_event(AppEvent::Tick);
    }
    assert!(app.toast.is_some());

    app.dispatch_event(AppEvent::Tick);
    assert!(app.toast.is_none());
}

#[test]
fn track_liked_event_refreshes_liked_songs_when_active() {
    let mut app = AppState::new();
    let track = dummy_track("soundcloud:tracks:15", "Refetch Me");
    app.session = Some(dummy_session());
    app.dispatch_event(AppEvent::LikedSongsLoaded {
        session: dummy_session(),
        page: Page {
            items: vec![track],
            next_href: None,
        },
        append: false,
    });
    app.set_route(Route::LikedSongs);
    while app.take_pending_command().is_some() {}

    app.dispatch_event(AppEvent::TrackLiked {
        session: dummy_session(),
        track_title: "Refetch Me".to_string(),
    });

    match app.take_pending_command() {
        Some(AppCommand::LoadLikedSongs { append, .. }) => assert!(!append),
        other => panic!("expected LoadLikedSongs command, got {other:?}"),
    }
    assert_eq!(
        app.toast.as_ref().expect("toast expected").message,
        "Added to Liked Songs"
    );
}

#[test]
fn track_added_to_playlist_refreshes_sidebar_and_active_playlist() {
    let mut app = AppState::new();
    let playlist = dummy_playlist("soundcloud:playlists:16", "Refresh Playlist");
    app.session = Some(dummy_session());

    app.dispatch_event(AppEvent::PlaylistsLoaded {
        session: dummy_session(),
        page: Page {
            items: vec![playlist.clone()],
            next_href: None,
        },
        append: false,
    });
    app.sync_route_from_playlist();
    while app.take_pending_command().is_some() {}

    app.dispatch_event(AppEvent::TrackAddedToPlaylist {
        session: dummy_session(),
        playlist_urn: playlist.urn.clone(),
        playlist_title: playlist.title.clone(),
        track_title: "Fresh Track".to_string(),
        already_present: false,
    });

    match app.take_pending_command() {
        Some(AppCommand::LoadPlaylists { append, .. }) => assert!(!append),
        other => panic!("expected LoadPlaylists command, got {other:?}"),
    }
    match app.take_pending_command() {
        Some(AppCommand::LoadPlaylistTracks {
            playlist_urn,
            append,
            ..
        }) => {
            assert_eq!(playlist_urn, playlist.urn);
            assert!(!append);
        }
        other => panic!("expected LoadPlaylistTracks command, got {other:?}"),
    }
    assert_eq!(
        app.toast.as_ref().expect("toast expected").message,
        "Added to playlist"
    );
}

#[test]
fn selecting_album_opens_playlist_detail_route() {
    let mut app = AppState::new();
    let playlist = dummy_playlist("soundcloud:playlists:2", "Weekend Album");
    app.session = Some(dummy_session());

    app.dispatch_event(AppEvent::AlbumsLoaded {
        session: dummy_session(),
        page: Page {
            items: vec![playlist.clone()],
            next_href: None,
        },
        append: false,
    });

    app.set_route(Route::Albums);
    app.select_current_content();

    assert_eq!(app.route, Route::Playlist);
    assert_eq!(app.route_title(), playlist.title);

    match app.take_pending_command() {
        Some(AppCommand::LoadPlaylistTracks { playlist_urn, .. }) => {
            assert_eq!(playlist_urn, "soundcloud:playlists:2")
        }
        other => panic!("expected LoadPlaylistTracks command, got {other:?}"),
    }
}

#[test]
fn selecting_user_opens_profile_route_and_queues_track_load() {
    let mut app = AppState::new();
    let user = dummy_user();
    app.session = Some(dummy_session());

    app.dispatch_event(AppEvent::FollowingLoaded {
        session: dummy_session(),
        page: Page {
            items: vec![user.clone()],
            next_href: None,
        },
        append: false,
    });

    app.set_route(Route::Following);
    while app.take_pending_command().is_some() {}

    app.select_current_content();

    assert_eq!(app.route, Route::UserProfile);
    assert_eq!(app.route_title(), user.username);
    assert_eq!(app.current_content().columns[0], "Title");

    match app.take_pending_command() {
        Some(AppCommand::LoadUserTracks { user_urn, .. }) => assert_eq!(user_urn, user.urn),
        other => panic!("expected LoadUserTracks command, got {other:?}"),
    }
}

#[test]
fn user_profile_shortcuts_switch_to_playlists_and_queue_load() {
    let mut app = AppState::new();
    let user = dummy_user();
    app.session = Some(dummy_session());

    app.dispatch_event(AppEvent::FollowingLoaded {
        session: dummy_session(),
        page: Page {
            items: vec![user.clone()],
            next_href: None,
        },
        append: false,
    });

    app.set_route(Route::Following);
    while app.take_pending_command().is_some() {}
    app.select_current_content();
    while app.take_pending_command().is_some() {}

    app.dispatch_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('2'),
        KeyModifiers::NONE,
    )));

    assert_eq!(app.route, Route::UserProfile);
    assert_eq!(app.current_content().columns[0], "Playlist");

    match app.take_pending_command() {
        Some(AppCommand::LoadUserPlaylists { user_urn, .. }) => assert_eq!(user_urn, user.urn),
        other => panic!("expected LoadUserPlaylists command, got {other:?}"),
    }
}

fn dummy_session() -> AuthorizedSession {
    AuthorizedSession {
        profile: AuthSession {
            username: "tester".to_string(),
            permalink_url: Some("https://soundcloud.com/tester".to_string()),
        },
        credentials: Credentials {
            client_id: "client-id".to_string(),
            client_secret: "client-secret".to_string(),
            redirect_uri: "http://127.0.0.1:8974/callback".to_string(),
        },
        tokens: TokenStore {
            access_token: "access-token".to_string(),
            refresh_token: "refresh-token".to_string(),
            token_type: "Bearer".to_string(),
            scope: None,
            expires_at_epoch: chrono::Utc::now().timestamp() + 3600,
        },
    }
}

fn drain_pending_commands(app: &mut AppState) {
    while app.take_pending_command().is_some() {}
}

fn seed_liked_tracks(app: &mut AppState, tracks: Vec<TrackSummary>) {
    app.dispatch_event(AppEvent::LikedSongsLoaded {
        session: dummy_session(),
        page: Page {
            items: tracks,
            next_href: None,
        },
        append: false,
    });
    app.set_route(Route::LikedSongs);
    drain_pending_commands(app);
    app.focus = Focus::Content;
    app.selected_content = 0;
}

fn seed_search_tracks(app: &mut AppState, query: &str, tracks: Vec<TrackSummary>) {
    app.search_query = query.to_string();
    app.search_cursor = query.chars().count();
    app.set_route(Route::Search);
    drain_pending_commands(app);
    app.dispatch_event(AppEvent::SearchLoaded {
        session: dummy_session(),
        query: query.to_string(),
        results: SearchResults {
            tracks: Page {
                items: tracks,
                next_href: None,
            },
            playlists: Page {
                items: Vec::new(),
                next_href: None,
            },
            users: Page {
                items: Vec::new(),
                next_href: None,
            },
        },
    });
    drain_pending_commands(app);
    app.focus = Focus::Content;
    app.selected_content = 0;
}

fn dummy_track(urn: &str, title: &str) -> TrackSummary {
    TrackSummary {
        urn: urn.to_string(),
        title: title.to_string(),
        artist: "Test Artist".to_string(),
        artist_urn: Some("soundcloud:users:1".to_string()),
        duration_ms: Some(200_000),
        permalink_url: Some(format!("https://soundcloud.com/tester/{title}")),
        artwork_url: None,
        access: None,
        streamable: true,
    }
}

fn dummy_playlist(urn: &str, title: &str) -> PlaylistSummary {
    PlaylistSummary {
        urn: urn.to_string(),
        title: title.to_string(),
        description: "A saved playlist".to_string(),
        creator: "Playlist Curator".to_string(),
        creator_urn: Some("soundcloud:users:1".to_string()),
        track_count: 12,
        duration_ms: Some(2_400_000),
        permalink_url: Some(format!("https://soundcloud.com/tester/sets/{title}")),
        artwork_url: None,
        playlist_type: Some("playlist".to_string()),
        release_year: Some(2024),
    }
}

fn dummy_user() -> UserSummary {
    UserSummary {
        urn: "soundcloud:users:2".to_string(),
        username: "Profile User".to_string(),
        permalink_url: Some("https://soundcloud.com/profile-user".to_string()),
        avatar_url: None,
        followers_count: 42_000,
        track_count: 8,
        playlist_count: 3,
    }
}
