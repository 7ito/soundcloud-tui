use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use soundcloud_tui::{
    app::{AppCommand, AppEvent, AppMode, AppState, AuthFocus, AuthStep, Route},
    config::{
        credentials::Credentials, history::RecentlyPlayedStore, settings::Settings,
        tokens::TokenStore,
    },
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
    app.queue.tracks = vec![first.clone(), second.clone()];
    app.queue.current_index = Some(0);
    app.now_playing.track = Some(first.clone());
    app.now_playing.title = first.title.clone();
    app.now_playing.artist = first.artist.clone();
    app.now_playing.context = "Liked Songs".to_string();

    app.dispatch_event(AppEvent::Player(PlayerEvent::TrackEnded));

    match app.take_pending_command() {
        Some(AppCommand::PlayTrack { track, .. }) => assert_eq!(track.title, second.title),
        other => panic!("expected PlayTrack command, got {other:?}"),
    }

    assert_eq!(app.queue.current_index, Some(1));
    assert_eq!(app.now_playing.title, second.title);
}

#[test]
fn auth_complete_shows_help_and_persists_dismissal() {
    let mut app = AppState::new_onboarding_with_persistence(
        Credentials::default(),
        Settings {
            theme: "default".to_string(),
            show_help_on_startup: true,
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
