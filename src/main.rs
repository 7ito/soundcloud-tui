use std::{
    io,
    io::{IsTerminal, Write},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use anyhow::Result;
use arboard::Clipboard;
use crossterm::{
    event::{DisableBracketedPaste, EnableBracketedPaste},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use log::{info, warn};
use ratatui::{Terminal, backend::CrosstermBackend};
#[cfg(all(feature = "mpris", target_os = "linux"))]
use soundcloud_tui::integrations::mpris::MprisIntegration;
use soundcloud_tui::{
    app::{AppCommand, AppEvent, AppState},
    config::{self, paths::AppPaths},
    input::events::EventHandler,
    player::{command::PlayerCommand, runtime::PlayerHandle},
    soundcloud::{auth, auth::AuthorizedSession, service::SoundcloudService},
    ui::{self, cover_art::CoverArtRenderer},
};
use tokio::{sync::mpsc, task::LocalSet};

#[tokio::main]
async fn main() {
    let local = LocalSet::new();

    if let Err(error) = local.run_until(run()).await {
        eprintln!("soundcloud-tui failed: {error:?}");
    }
}

async fn run() -> Result<()> {
    if !io::stdout().is_terminal() {
        anyhow::bail!("soundcloud-tui must be run in an interactive terminal");
    }

    let paths = AppPaths::discover()?;
    paths.ensure_dirs()?;
    config::settings::ensure_default_file(&paths)?;
    config::init_logging(&paths)?;
    let settings = config::settings::Settings::load(&paths)?;
    let recent_history = config::history::RecentlyPlayedStore::load(&paths)?;

    info!("starting soundcloud-tui auth onboarding scaffold");

    let bootstrap = auth::bootstrap(&paths);

    let mut terminal = TerminalHandle::new()?;
    let mut app = AppState::new_onboarding_with_persistence(
        bootstrap.credentials.clone(),
        settings,
        recent_history,
    );
    if let Some(warning) = bootstrap.warning {
        app.auth.set_error(warning.clone());
        app.status = warning;
    }

    if let Some(tokens) = bootstrap.tokens {
        let credentials = app.auth.credentials();
        if credentials.validate().is_ok() {
            app.begin_saved_session_validation(credentials, tokens);
        }
    }

    let mut events = EventHandler::new(Duration::from_millis(250));
    let (async_tx, mut async_rx) = mpsc::unbounded_channel::<AppEvent>();
    let player = PlayerHandle::spawn(paths.clone(), async_tx.clone());

    #[cfg(all(feature = "mpris", target_os = "linux"))]
    let mut mpris = match MprisIntegration::new(async_tx.clone()).await {
        Ok(mut integration) => {
            if let Err(error) = integration.sync_from_app(&app).await {
                warn!("disabling MPRIS integration after initial sync failure: {error}");
                None
            } else {
                Some(integration)
            }
        }
        Err(error) => {
            warn!("MPRIS integration unavailable: {error}");
            None
        }
    };

    loop {
        drain_commands(&mut app, &paths, &async_tx, &player);
        terminal.draw(&app)?;

        tokio::select! {
            maybe_event = events.next() => {
                let Some(event) = maybe_event else { break; };
                app.dispatch_event(event);
            }
            maybe_async = async_rx.recv() => {
                let Some(event) = maybe_async else { break; };
                app.dispatch_event(event);
            }
        }

        #[cfg(all(feature = "mpris", target_os = "linux"))]
        if let Some(integration) = mpris.as_mut() {
            if let Err(error) = integration.sync_from_app(&app).await {
                warn!("disabling MPRIS integration after sync failure: {error}");
                mpris = None;
            }
        }

        if app.should_quit {
            break;
        }
    }

    info!("shutting down soundcloud-tui");

    Ok(())
}

fn drain_commands(
    app: &mut AppState,
    paths: &AppPaths,
    sender: &mpsc::UnboundedSender<AppEvent>,
    player: &PlayerHandle,
) {
    while let Some(command) = app.take_pending_command() {
        run_command(command, paths.clone(), sender.clone(), player.clone());
    }
}

fn run_command(
    command: AppCommand,
    paths: AppPaths,
    sender: mpsc::UnboundedSender<AppEvent>,
    player: PlayerHandle,
) {
    match command {
        AppCommand::OpenUrl(url) => {
            if let Err(error) = open::that(url.as_str()) {
                warn!("failed to open URL in browser: {error}");
            }
        }
        AppCommand::SaveCredentials(request) => {
            let result = request.credentials.save(&paths);
            let _ = match result {
                Ok(()) => sender.send(AppEvent::CredentialsSaved(request)),
                Err(error) => sender.send(AppEvent::CredentialsSaveFailed(error.to_string())),
            };
        }
        AppCommand::SaveSettings(settings) => {
            if let Err(error) = settings.save(&paths) {
                warn!("failed to save settings: {error}");
            }
        }
        AppCommand::SaveHistory(history) => {
            if let Err(error) = history.save(&paths) {
                warn!("failed to save playback history: {error}");
            }
        }
        AppCommand::ValidateSavedSession {
            credentials,
            tokens,
        } => {
            tokio::spawn(async move {
                let result = auth::restore_saved_session(&paths, &credentials, &tokens)
                    .await
                    .map_err(|error| error.to_string());
                let _ = sender.send(AppEvent::AuthRestoreComplete(result));
            });
        }
        AppCommand::WaitForOAuthCallback(request) => {
            tokio::spawn(async move {
                let result =
                    auth::wait_for_callback(&request.credentials.redirect_uri, &request.state)
                        .await;
                let _ = match result {
                    Ok(callback_input) => {
                        sender.send(AppEvent::AuthCallbackCaptured(callback_input))
                    }
                    Err(error) => sender.send(AppEvent::AuthCallbackFailed(error.to_string())),
                };
            });
        }
        AppCommand::ExchangeAuthorizationCode {
            request,
            callback_input,
        } => {
            tokio::spawn(async move {
                let result = auth::complete_authorization(&paths, &request, &callback_input)
                    .await
                    .map_err(|error| error.to_string());
                let _ = sender.send(AppEvent::AuthCompleted(result));
            });
        }
        AppCommand::LoadFeed {
            session,
            next_href,
            append,
        } => {
            tokio::spawn(async move {
                let result =
                    execute_session_command(paths, session, move |service, session| async move {
                        let page = service
                            .load_feed(&session.tokens.access_token, next_href.as_deref())
                            .await?;
                        Ok(AppEvent::FeedLoaded {
                            session,
                            page,
                            append,
                        })
                    })
                    .await;
                let _ = sender.send(match result {
                    Ok(event) => event,
                    Err(error) => AppEvent::FeedFailed(error.to_string()),
                });
            });
        }
        AppCommand::LoadLikedSongs {
            session,
            next_href,
            append,
        } => {
            tokio::spawn(async move {
                let result =
                    execute_session_command(paths, session, move |service, session| async move {
                        let page = service
                            .load_liked_tracks(&session.tokens.access_token, next_href.as_deref())
                            .await?;
                        Ok(AppEvent::LikedSongsLoaded {
                            session,
                            page,
                            append,
                        })
                    })
                    .await;
                let _ = sender.send(match result {
                    Ok(event) => event,
                    Err(error) => AppEvent::LikedSongsFailed(error.to_string()),
                });
            });
        }
        AppCommand::LoadAlbums {
            session,
            next_href,
            append,
        } => {
            tokio::spawn(async move {
                let result =
                    execute_session_command(paths, session, move |service, session| async move {
                        let page = service
                            .load_albums(&session.tokens.access_token, next_href.as_deref())
                            .await?;
                        Ok(AppEvent::AlbumsLoaded {
                            session,
                            page,
                            append,
                        })
                    })
                    .await;
                let _ = sender.send(match result {
                    Ok(event) => event,
                    Err(error) => AppEvent::AlbumsFailed(error.to_string()),
                });
            });
        }
        AppCommand::LoadFollowing {
            session,
            next_href,
            append,
        } => {
            tokio::spawn(async move {
                let result =
                    execute_session_command(paths, session, move |service, session| async move {
                        let page = service
                            .load_followings(&session.tokens.access_token, next_href.as_deref())
                            .await?;
                        Ok(AppEvent::FollowingLoaded {
                            session,
                            page,
                            append,
                        })
                    })
                    .await;
                let _ = sender.send(match result {
                    Ok(event) => event,
                    Err(error) => AppEvent::FollowingFailed(error.to_string()),
                });
            });
        }
        AppCommand::LoadPlaylists {
            session,
            next_href,
            append,
        } => {
            tokio::spawn(async move {
                let result =
                    execute_session_command(paths, session, move |service, session| async move {
                        let page = service
                            .load_playlists(&session.tokens.access_token, next_href.as_deref())
                            .await?;
                        Ok(AppEvent::PlaylistsLoaded {
                            session,
                            page,
                            append,
                        })
                    })
                    .await;
                let _ = sender.send(match result {
                    Ok(event) => event,
                    Err(error) => AppEvent::PlaylistsFailed(error.to_string()),
                });
            });
        }
        AppCommand::LoadPlaylistTracks {
            session,
            playlist_urn,
            next_href,
            append,
        } => {
            tokio::spawn(async move {
                let failed_playlist_urn = playlist_urn.clone();
                let result =
                    execute_session_command(paths, session, move |service, session| async move {
                        let page = service
                            .load_playlist_tracks(
                                &session.tokens.access_token,
                                &playlist_urn,
                                next_href.as_deref(),
                            )
                            .await?;
                        Ok(AppEvent::PlaylistTracksLoaded {
                            session,
                            playlist_urn,
                            page,
                            append,
                        })
                    })
                    .await;
                let _ = sender.send(match result {
                    Ok(event) => event,
                    Err(error) => AppEvent::PlaylistTracksFailed {
                        playlist_urn: failed_playlist_urn,
                        error: error.to_string(),
                    },
                });
            });
        }
        AppCommand::LoadUserTracks {
            session,
            user_urn,
            next_href,
            append,
        } => {
            tokio::spawn(async move {
                let failed_user_urn = user_urn.clone();
                let result =
                    execute_session_command(paths, session, move |service, session| async move {
                        let page = service
                            .load_user_tracks(
                                &session.tokens.access_token,
                                &user_urn,
                                next_href.as_deref(),
                            )
                            .await?;
                        Ok(AppEvent::UserTracksLoaded {
                            session,
                            user_urn,
                            page,
                            append,
                        })
                    })
                    .await;
                let _ = sender.send(match result {
                    Ok(event) => event,
                    Err(error) => AppEvent::UserTracksFailed {
                        user_urn: failed_user_urn,
                        error: error.to_string(),
                    },
                });
            });
        }
        AppCommand::LoadUserPlaylists {
            session,
            user_urn,
            next_href,
            append,
        } => {
            tokio::spawn(async move {
                let failed_user_urn = user_urn.clone();
                let result =
                    execute_session_command(paths, session, move |service, session| async move {
                        let page = service
                            .load_user_playlists(
                                &session.tokens.access_token,
                                &user_urn,
                                next_href.as_deref(),
                            )
                            .await?;
                        Ok(AppEvent::UserPlaylistsLoaded {
                            session,
                            user_urn,
                            page,
                            append,
                        })
                    })
                    .await;
                let _ = sender.send(match result {
                    Ok(event) => event,
                    Err(error) => AppEvent::UserPlaylistsFailed {
                        user_urn: failed_user_urn,
                        error: error.to_string(),
                    },
                });
            });
        }
        AppCommand::SearchAll { session, query } => {
            tokio::spawn(async move {
                let query_for_error = query.clone();
                let result =
                    execute_session_command(paths, session, move |service, session| async move {
                        let results = service
                            .search_all(&session.tokens.access_token, &query)
                            .await?;
                        Ok(AppEvent::SearchLoaded {
                            session,
                            query,
                            results,
                        })
                    })
                    .await;
                let _ = sender.send(match result {
                    Ok(event) => event,
                    Err(error) => AppEvent::SearchFailed {
                        query: query_for_error,
                        error: error.to_string(),
                    },
                });
            });
        }
        AppCommand::SearchTracksPage {
            session,
            query,
            next_href,
        } => {
            tokio::spawn(async move {
                let query_for_error = query.clone();
                let result =
                    execute_session_command(paths, session, move |service, session| async move {
                        let page = service
                            .search_tracks(&session.tokens.access_token, &query, Some(&next_href))
                            .await?;
                        Ok(AppEvent::SearchTracksPageLoaded {
                            session,
                            query,
                            page,
                        })
                    })
                    .await;
                let _ = sender.send(match result {
                    Ok(event) => event,
                    Err(error) => AppEvent::SearchTracksPageFailed {
                        query: query_for_error,
                        error: error.to_string(),
                    },
                });
            });
        }
        AppCommand::CopyText { text, label } => {
            tokio::task::spawn_blocking(move || {
                let event = match copy_text_to_clipboard(&text) {
                    Ok(()) => AppEvent::ClipboardCopied { label },
                    Err(error) => AppEvent::ClipboardCopyFailed { label, error },
                };
                let _ = sender.send(event);
            });
        }
        AppCommand::LoadCoverArt { url } => {
            tokio::spawn(async move {
                let url_for_error = url.clone();
                let result = async {
                    let response = reqwest::get(&url).await?.error_for_status()?;
                    let bytes = response.bytes().await?;
                    Ok::<_, reqwest::Error>(AppEvent::CoverArtLoaded {
                        url,
                        bytes: bytes.to_vec(),
                    })
                }
                .await;

                let _ = sender.send(match result {
                    Ok(event) => event,
                    Err(error) => AppEvent::CoverArtFailed {
                        url: url_for_error,
                        error: error.to_string(),
                    },
                });
            });
        }
        AppCommand::PlayTrack { session, track } => {
            tokio::spawn(async move {
                let title = track.title.clone();
                let result =
                    execute_session_command(paths, session, move |service, session| async move {
                        let stream = service
                            .resolve_stream(&session.tokens.access_token, &track)
                            .await?;
                        player.send(PlayerCommand::LoadTrack {
                            url: stream.url,
                            title: track.title.clone(),
                            authorization: Some(session.tokens.access_token.clone()),
                        })?;
                        Ok(AppEvent::PlaybackQueued {
                            session,
                            title: track.title,
                            preview: stream.preview,
                        })
                    })
                    .await;

                let _ = sender.send(match result {
                    Ok(event) => event,
                    Err(error) => AppEvent::PlaybackFailed {
                        title,
                        error: error.to_string(),
                    },
                });
            });
        }
        AppCommand::ControlPlayback(command) => {
            if let Err(error) = player.send(command) {
                let _ = sender.send(AppEvent::Player(
                    soundcloud_tui::player::event::PlayerEvent::BackendError(error.to_string()),
                ));
            }
        }
    }
}

async fn execute_session_command<F, Fut>(
    paths: AppPaths,
    mut session: AuthorizedSession,
    run: F,
) -> Result<AppEvent>
where
    F: FnOnce(SoundcloudService, AuthorizedSession) -> Fut,
    Fut: std::future::Future<Output = Result<AppEvent>>,
{
    session.tokens =
        auth::ensure_fresh_tokens(&paths, &session.credentials, &session.tokens).await?;
    let service = SoundcloudService::new()?;
    run(service, session).await
}

fn copy_text_to_clipboard(text: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let mut errors = Vec::new();

        for candidate in linux_clipboard_candidates() {
            match run_linux_clipboard_command(candidate, text) {
                Ok(()) => return Ok(()),
                Err(error) => errors.push(error),
            }
        }

        use arboard::{LinuxClipboardKind, SetExtLinux};

        let mut clipboard = Clipboard::new().map_err(|error| {
            errors.push(format!("arboard init: {error}"));
            errors.join("; ")
        })?;

        return clipboard
            .set()
            .clipboard(LinuxClipboardKind::Clipboard)
            .wait_until(Instant::now() + Duration::from_millis(250))
            .text(text.to_string())
            .map_err(|error| {
                errors.push(format!("arboard: {error}"));
                errors.join("; ")
            });
    }

    #[cfg(not(target_os = "linux"))]
    {
        let mut clipboard = Clipboard::new().map_err(|error| error.to_string())?;
        clipboard
            .set_text(text.to_string())
            .map_err(|error| error.to_string())
    }
}

#[cfg(target_os = "linux")]
fn linux_clipboard_candidates() -> Vec<(&'static str, &'static [&'static str])> {
    let mut candidates = Vec::new();

    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        candidates.push(("wl-copy", &["--type", "text/plain"][..]));
    }
    if std::env::var_os("DISPLAY").is_some() {
        candidates.push(("xclip", &["-selection", "clipboard", "-in"][..]));
        candidates.push(("xsel", &["--clipboard", "--input"][..]));
    }
    if candidates.is_empty() {
        candidates.push(("wl-copy", &["--type", "text/plain"][..]));
        candidates.push(("xclip", &["-selection", "clipboard", "-in"][..]));
        candidates.push(("xsel", &["--clipboard", "--input"][..]));
    }

    candidates
}

#[cfg(target_os = "linux")]
fn run_linux_clipboard_command(
    candidate: (&'static str, &'static [&'static str]),
    text: &str,
) -> Result<(), String> {
    let (program, args) = candidate;
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("{program}: {error}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|error| format!("{program}: failed writing stdin: {error}"))?;
    }

    let status = child
        .wait()
        .map_err(|error| format!("{program}: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("{program}: exited with status {status}"))
    }
}

struct TerminalHandle {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    cover_art: CoverArtRenderer,
}

impl TerminalHandle {
    fn new() -> Result<Self> {
        enable_raw_mode()?;

        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableBracketedPaste)?;

        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        let cover_art = CoverArtRenderer::new();

        Ok(Self {
            terminal,
            cover_art,
        })
    }

    fn draw(&mut self, app: &AppState) -> Result<()> {
        self.terminal
            .draw(|frame| ui::layout::render_app(frame, app, &mut self.cover_art))?;
        Ok(())
    }
}

impl Drop for TerminalHandle {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            DisableBracketedPaste,
            LeaveAlternateScreen
        );
        let _ = self.terminal.show_cursor();
    }
}
