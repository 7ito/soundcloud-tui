use std::{
    future::Future,
    io::{self, Write},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use anyhow::Result;
use arboard::Clipboard;
use log::warn;
use tokio::{sync::mpsc, task};

use crate::{
    app::{AppCommand, AppEvent, AppState},
    config::paths::AppPaths,
    player::{event::PlayerEvent, runtime::PlayerHandle},
    soundcloud::{
        auth,
        auth::AuthorizedSession,
        service::{PlaylistTrackAddResult, SoundcloudService},
    },
    visualizer::VisualizerHandle,
};

#[derive(Clone)]
pub struct CommandExecutor {
    paths: AppPaths,
    sender: mpsc::UnboundedSender<AppEvent>,
    player: PlayerHandle,
    visualizer: VisualizerHandle,
}

impl CommandExecutor {
    pub fn new(
        paths: AppPaths,
        sender: mpsc::UnboundedSender<AppEvent>,
        player: PlayerHandle,
        visualizer: VisualizerHandle,
    ) -> Self {
        Self {
            paths,
            sender,
            player,
            visualizer,
        }
    }

    pub fn drain(&self, app: &mut AppState) {
        while let Some(command) = app.take_pending_command() {
            self.run(command);
        }
    }

    fn run(&self, command: AppCommand) {
        match command {
            AppCommand::OpenUrl(url) => {
                if let Err(error) = open::that(url.as_str()) {
                    warn!("failed to open URL in browser: {error}");
                }
            }
            AppCommand::SaveCredentials(request) => {
                let result = request.credentials.save(&self.paths);
                let _ = match result {
                    Ok(()) => self.sender.send(AppEvent::CredentialsSaved(request)),
                    Err(error) => self
                        .sender
                        .send(AppEvent::CredentialsSaveFailed(error.to_string())),
                };
            }
            AppCommand::SaveSettings(settings) => {
                if let Err(error) = settings.save(&self.paths) {
                    warn!("failed to save settings: {error}");
                }
            }
            AppCommand::SetWindowTitle(title) => {
                if let Err(error) =
                    crossterm::execute!(io::stdout(), crossterm::terminal::SetTitle(title))
                {
                    warn!("failed to set window title: {error}");
                }
            }
            AppCommand::SaveHistory(history) => {
                if let Err(error) = history.save(&self.paths) {
                    warn!("failed to save playback history: {error}");
                }
            }
            AppCommand::ValidateSavedSession {
                credentials,
                tokens,
            } => {
                let paths = self.paths.clone();
                let sender = self.sender.clone();
                tokio::spawn(async move {
                    let result = auth::restore_saved_session(&paths, &credentials, &tokens)
                        .await
                        .map_err(|error| error.to_string());
                    let _ = sender.send(AppEvent::AuthRestoreComplete(result));
                });
            }
            AppCommand::WaitForOAuthCallback(request) => {
                let sender = self.sender.clone();
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
                let paths = self.paths.clone();
                let sender = self.sender.clone();
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
            } => self.spawn_session_event(
                session,
                |error| AppEvent::FeedFailed(error.to_string()),
                move |service, session| async move {
                    let page = service
                        .load_feed(&session.tokens.access_token, next_href.as_deref())
                        .await?;
                    Ok(AppEvent::FeedLoaded {
                        session,
                        page,
                        append,
                    })
                },
            ),
            AppCommand::LoadLikedSongs {
                session,
                next_href,
                append,
            } => self.spawn_session_event(
                session,
                |error| AppEvent::LikedSongsFailed(error.to_string()),
                move |service, session| async move {
                    let page = service
                        .load_liked_tracks(&session.tokens.access_token, next_href.as_deref())
                        .await?;
                    Ok(AppEvent::LikedSongsLoaded {
                        session,
                        page,
                        append,
                    })
                },
            ),
            AppCommand::LoadAlbums {
                session,
                next_href,
                append,
            } => self.spawn_session_event(
                session,
                |error| AppEvent::AlbumsFailed(error.to_string()),
                move |service, session| async move {
                    let page = service
                        .load_albums(&session.tokens.access_token, next_href.as_deref())
                        .await?;
                    Ok(AppEvent::AlbumsLoaded {
                        session,
                        page,
                        append,
                    })
                },
            ),
            AppCommand::LoadFollowing {
                session,
                next_href,
                append,
            } => self.spawn_session_event(
                session,
                |error| AppEvent::FollowingFailed(error.to_string()),
                move |service, session| async move {
                    let page = service
                        .load_followings(&session.tokens.access_token, next_href.as_deref())
                        .await?;
                    Ok(AppEvent::FollowingLoaded {
                        session,
                        page,
                        append,
                    })
                },
            ),
            AppCommand::LoadPlaylists {
                session,
                next_href,
                append,
            } => self.spawn_session_event(
                session,
                |error| AppEvent::PlaylistsFailed(error.to_string()),
                move |service, session| async move {
                    let page = service
                        .load_playlists(&session.tokens.access_token, next_href.as_deref())
                        .await?;
                    Ok(AppEvent::PlaylistsLoaded {
                        session,
                        page,
                        append,
                    })
                },
            ),
            AppCommand::LoadPlaylistTracks {
                session,
                playlist_urn,
                next_href,
                append,
            } => {
                let failed_playlist_urn = playlist_urn.clone();
                self.spawn_session_event(
                    session,
                    move |error| AppEvent::PlaylistTracksFailed {
                        playlist_urn: failed_playlist_urn,
                        error: error.to_string(),
                    },
                    move |service, session| async move {
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
                    },
                );
            }
            AppCommand::LoadUserTracks {
                session,
                user_urn,
                next_href,
                append,
            } => {
                let failed_user_urn = user_urn.clone();
                self.spawn_session_event(
                    session,
                    move |error| AppEvent::UserTracksFailed {
                        user_urn: failed_user_urn,
                        error: error.to_string(),
                    },
                    move |service, session| async move {
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
                    },
                );
            }
            AppCommand::LoadUserPlaylists {
                session,
                user_urn,
                next_href,
                append,
            } => {
                let failed_user_urn = user_urn.clone();
                self.spawn_session_event(
                    session,
                    move |error| AppEvent::UserPlaylistsFailed {
                        user_urn: failed_user_urn,
                        error: error.to_string(),
                    },
                    move |service, session| async move {
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
                    },
                );
            }
            AppCommand::SearchAll { session, query } => {
                let query_for_error = query.clone();
                self.spawn_session_event(
                    session,
                    move |error| AppEvent::SearchFailed {
                        query: query_for_error,
                        error: error.to_string(),
                    },
                    move |service, session| async move {
                        let results = service
                            .search_all(&session.tokens.access_token, &query)
                            .await?;
                        Ok(AppEvent::SearchLoaded {
                            session,
                            query,
                            results,
                        })
                    },
                );
            }
            AppCommand::SearchTracksPage {
                session,
                query,
                next_href,
            } => {
                let query_for_error = query.clone();
                self.spawn_session_event(
                    session,
                    move |error| AppEvent::SearchTracksPageFailed {
                        query: query_for_error,
                        error: error.to_string(),
                    },
                    move |service, session| async move {
                        let page = service
                            .search_tracks(&session.tokens.access_token, &query, Some(&next_href))
                            .await?;
                        Ok(AppEvent::SearchTracksPageLoaded {
                            session,
                            query,
                            page,
                        })
                    },
                );
            }
            AppCommand::LikeTrack { session, track } => {
                let track_title = track.title.clone();
                self.spawn_session_event(
                    session,
                    move |error| AppEvent::TrackLikeFailed {
                        track_title,
                        error: error.to_string(),
                    },
                    move |service, session| async move {
                        service
                            .like_track(&session.tokens.access_token, &track)
                            .await?;
                        Ok(AppEvent::TrackLiked {
                            session,
                            track_title: track.title,
                        })
                    },
                );
            }
            AppCommand::AddTrackToPlaylist {
                session,
                track,
                playlist,
            } => {
                let track_title = track.title.clone();
                let playlist_title = playlist.title.clone();
                let playlist_urn = playlist.urn.clone();
                self.spawn_session_event(
                    session,
                    move |error| AppEvent::TrackAddToPlaylistFailed {
                        playlist_title,
                        track_title,
                        error: error.to_string(),
                    },
                    move |service, session| async move {
                        let outcome = service
                            .add_track_to_playlist(&session.tokens.access_token, &playlist, &track)
                            .await?;
                        Ok(AppEvent::TrackAddedToPlaylist {
                            session,
                            playlist_urn,
                            playlist_title: playlist.title,
                            track_title: track.title,
                            already_present: outcome == PlaylistTrackAddResult::AlreadyPresent,
                        })
                    },
                );
            }
            AppCommand::CopyText { text, label } => {
                let sender = self.sender.clone();
                task::spawn_blocking(move || {
                    let event = match copy_text_to_clipboard(&text) {
                        Ok(()) => AppEvent::ClipboardCopied { label },
                        Err(error) => AppEvent::ClipboardCopyFailed { label, error },
                    };
                    let _ = sender.send(event);
                });
            }
            AppCommand::LoadCoverArt { url } => {
                let sender = self.sender.clone();
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
                let player = self.player.clone();
                let title = track.title.clone();
                self.spawn_session_event(
                    session,
                    move |error| AppEvent::PlaybackFailed {
                        title,
                        error: error.to_string(),
                    },
                    move |service, session| async move {
                        let stream = service
                            .resolve_stream(&session.tokens.access_token, &track)
                            .await?;
                        player.send(crate::player::command::PlayerCommand::LoadTrack {
                            url: stream.url,
                            title: track.title.clone(),
                            authorization: Some(session.tokens.access_token.clone()),
                        })?;
                        Ok(AppEvent::PlaybackQueued {
                            session,
                            title: track.title,
                            preview: stream.preview,
                        })
                    },
                );
            }
            AppCommand::ControlPlayback(command) => {
                if let Err(error) = self.player.send(command) {
                    let _ = self.sender.send(AppEvent::Player(PlayerEvent::BackendError(
                        error.to_string(),
                    )));
                }
            }
            AppCommand::ControlVisualizer(command) => {
                if let Err(error) = self.visualizer.send(command) {
                    let _ = self
                        .sender
                        .send(AppEvent::VisualizerCaptureFailed(error.to_string()));
                }
            }
        }
    }

    fn spawn_session_event<F, Fut, E>(&self, session: AuthorizedSession, on_error: E, run: F)
    where
        F: FnOnce(SoundcloudService, AuthorizedSession) -> Fut + Send + 'static,
        Fut: Future<Output = Result<AppEvent>> + Send + 'static,
        E: FnOnce(anyhow::Error) -> AppEvent + Send + 'static,
    {
        let paths = self.paths.clone();
        let sender = self.sender.clone();
        tokio::spawn(async move {
            let event = match execute_session_command(paths, session, run).await {
                Ok(event) => event,
                Err(error) => on_error(error),
            };
            let _ = sender.send(event);
        });
    }
}

async fn execute_session_command<F, Fut>(
    paths: AppPaths,
    mut session: AuthorizedSession,
    run: F,
) -> Result<AppEvent>
where
    F: FnOnce(SoundcloudService, AuthorizedSession) -> Fut,
    Fut: Future<Output = Result<AppEvent>>,
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
