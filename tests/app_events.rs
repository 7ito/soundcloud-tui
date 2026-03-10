use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use soundcloud_tui::{
    app::{AppCommand, AppEvent, AppState, AuthFocus, AuthStep, Route},
    config::{credentials::Credentials, tokens::TokenStore},
    player::event::PlayerEvent,
    soundcloud::{
        auth::{self, AuthSession, AuthorizedSession},
        models::TrackSummary,
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
