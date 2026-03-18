use std::{future::Future, io};

use anyhow::Result;
use log::warn;
use tokio::{sync::mpsc, task};

use crate::{
    app::{AppCommand, AppEvent, AppState},
    config::paths::AppPaths,
    player::{command::PlayerCommand, event::PlayerEvent, runtime::PlayerHandle},
    soundcloud::{
        auth,
        auth::AuthorizedSession,
        models::{PlaylistSummary, TrackSummary},
        service::{PlaylistTrackAddResult, SoundcloudService},
    },
    visualizer::{VisualizerCommand, VisualizerHandle},
};

mod clipboard;
mod session;

use clipboard::copy_text;

#[derive(Clone)]
pub struct CommandExecutor {
    paths: AppPaths,
    sender: mpsc::UnboundedSender<AppEvent>,
    player: PlayerHandle,
    visualizer: VisualizerHandle,
    service: SoundcloudService,
}

impl CommandExecutor {
    pub fn new(
        paths: AppPaths,
        sender: mpsc::UnboundedSender<AppEvent>,
        player: PlayerHandle,
        visualizer: VisualizerHandle,
    ) -> Result<Self> {
        Ok(Self {
            paths,
            sender,
            player,
            visualizer,
            service: SoundcloudService::new()?,
        })
    }

    pub fn drain(&self, app: &mut AppState) {
        while let Some(command) = app.take_pending_command() {
            self.run(command);
        }
    }

    fn run(&self, command: AppCommand) {
        match command {
            AppCommand::OpenUrl(url) => self.open_url(url),
            AppCommand::SaveCredentials(request) => self.save_credentials(request),
            AppCommand::SaveSettings(settings) => self.save_settings(settings),
            AppCommand::SetWindowTitle(title) => self.set_window_title(title),
            AppCommand::SaveHistory(history) => self.save_history(history),
            AppCommand::ValidateSavedSession {
                credentials,
                tokens,
            } => self.validate_saved_session(credentials, tokens),
            AppCommand::WaitForOAuthCallback(request) => self.wait_for_oauth_callback(request),
            AppCommand::ExchangeAuthorizationCode {
                request,
                callback_input,
            } => self.exchange_authorization_code(request, callback_input),
            AppCommand::Logout => self.logout(),
            AppCommand::LoadFeed {
                session,
                request_id,
                next_href,
                append,
            } => self.load_feed(session, request_id, next_href, append),
            AppCommand::LoadLikedSongs {
                session,
                request_id,
                next_href,
                append,
            } => self.load_liked_songs(session, request_id, next_href, append),
            AppCommand::LoadAlbums {
                session,
                request_id,
                next_href,
                append,
            } => self.load_albums(session, request_id, next_href, append),
            AppCommand::LoadFollowing {
                session,
                request_id,
                next_href,
                append,
            } => self.load_following(session, request_id, next_href, append),
            AppCommand::LoadPlaylists {
                session,
                request_id,
                next_href,
                append,
            } => self.load_playlists(session, request_id, next_href, append),
            AppCommand::LoadPlaylistTracks {
                session,
                request_id,
                playlist_urn,
                next_href,
                append,
            } => self.load_playlist_tracks(session, request_id, playlist_urn, next_href, append),
            AppCommand::LoadUserTracks {
                session,
                request_id,
                user_urn,
                next_href,
                append,
            } => self.load_user_tracks(session, request_id, user_urn, next_href, append),
            AppCommand::LoadUserPlaylists {
                session,
                request_id,
                user_urn,
                next_href,
                append,
            } => self.load_user_playlists(session, request_id, user_urn, next_href, append),
            AppCommand::SearchAll {
                session,
                request_id,
                query,
            } => self.search_all(session, request_id, query),
            AppCommand::SearchTracksPage {
                session,
                request_id,
                query,
                next_href,
            } => self.search_tracks_page(session, request_id, query, next_href),
            AppCommand::LikeTrack { session, track } => self.like_track(session, track),
            AppCommand::AddTrackToPlaylist {
                session,
                track,
                playlist,
            } => self.add_track_to_playlist(session, track, playlist),
            AppCommand::CopyText { text, label } => self.copy_text(text, label),
            AppCommand::LoadCoverArt { url } => self.load_cover_art(url),
            AppCommand::PlayTrack { session, track } => self.play_track(session, track),
            AppCommand::ControlPlayback(command) => self.control_playback(command),
            AppCommand::ControlVisualizer(command) => self.control_visualizer(command),
        }
    }

    fn open_url(&self, url: String) {
        if let Err(error) = open::that(url.as_str()) {
            warn!("failed to open URL in browser: {error}");
        }
    }

    fn save_credentials(&self, request: auth::AuthorizationRequest) {
        let result = request.credentials.save(&self.paths);
        let _ = match result {
            Ok(()) => self.sender.send(AppEvent::CredentialsSaved(request)),
            Err(error) => self
                .sender
                .send(AppEvent::CredentialsSaveFailed(format_error(&error))),
        };
    }

    fn save_settings(&self, settings: crate::config::settings::Settings) {
        if let Err(error) = settings.save(&self.paths) {
            warn!("failed to save settings: {error}");
        }
    }

    fn set_window_title(&self, title: String) {
        if let Err(error) = crossterm::execute!(io::stdout(), crossterm::terminal::SetTitle(title))
        {
            warn!("failed to set window title: {error}");
        }
    }

    fn save_history(&self, history: crate::config::history::RecentlyPlayedStore) {
        if let Err(error) = history.save(&self.paths) {
            warn!("failed to save playback history: {error}");
        }
    }

    fn validate_saved_session(
        &self,
        credentials: crate::config::credentials::Credentials,
        tokens: crate::config::tokens::TokenStore,
    ) {
        let paths = self.paths.clone();
        self.spawn_event_task(async move {
            let result = auth::restore_saved_session(&paths, &credentials, &tokens)
                .await
                .map_err(|error| format_error(&error));
            AppEvent::AuthRestoreComplete(result)
        });
    }

    fn wait_for_oauth_callback(&self, request: auth::AuthorizationRequest) {
        self.spawn_event_task(async move {
            match auth::wait_for_callback(&request.credentials.redirect_uri, &request.state).await {
                Ok(callback_input) => AppEvent::AuthCallbackCaptured(callback_input),
                Err(error) => AppEvent::AuthCallbackFailed(format_error(&error)),
            }
        });
    }

    fn exchange_authorization_code(
        &self,
        request: auth::AuthorizationRequest,
        callback_input: String,
    ) {
        let paths = self.paths.clone();
        self.spawn_event_task(async move {
            let result = auth::complete_authorization(&paths, &request, &callback_input)
                .await
                .map_err(|error| format_error(&error));
            AppEvent::AuthCompleted(result)
        });
    }

    fn logout(&self) {
        let result = crate::config::tokens::TokenStore::clear(&self.paths);
        let _ = match result {
            Ok(()) => self.sender.send(AppEvent::LogoutCompleted),
            Err(error) => self
                .sender
                .send(AppEvent::LogoutFailed(format_error(&error))),
        };
    }

    fn load_feed(
        &self,
        session: AuthorizedSession,
        request_id: u64,
        next_href: Option<String>,
        append: bool,
    ) {
        self.spawn_session_event(
            session,
            move |error| AppEvent::FeedFailed {
                request_id,
                error: error.to_string(),
            },
            move |service, session| async move {
                let page = service
                    .load_feed(&session.tokens.access_token, next_href.as_deref())
                    .await?;
                Ok(AppEvent::FeedLoaded {
                    session,
                    request_id,
                    page,
                    append,
                })
            },
        );
    }

    fn load_liked_songs(
        &self,
        session: AuthorizedSession,
        request_id: u64,
        next_href: Option<String>,
        append: bool,
    ) {
        self.spawn_session_event(
            session,
            move |error| AppEvent::LikedSongsFailed {
                request_id,
                error: error.to_string(),
            },
            move |service, session| async move {
                let page = service
                    .load_liked_tracks(&session.tokens.access_token, next_href.as_deref())
                    .await?;
                Ok(AppEvent::LikedSongsLoaded {
                    session,
                    request_id,
                    page,
                    append,
                })
            },
        );
    }

    fn load_albums(
        &self,
        session: AuthorizedSession,
        request_id: u64,
        next_href: Option<String>,
        append: bool,
    ) {
        self.spawn_session_event(
            session,
            move |error| AppEvent::AlbumsFailed {
                request_id,
                error: error.to_string(),
            },
            move |service, session| async move {
                let page = service
                    .load_albums(&session.tokens.access_token, next_href.as_deref())
                    .await?;
                Ok(AppEvent::AlbumsLoaded {
                    session,
                    request_id,
                    page,
                    append,
                })
            },
        );
    }

    fn load_following(
        &self,
        session: AuthorizedSession,
        request_id: u64,
        next_href: Option<String>,
        append: bool,
    ) {
        self.spawn_session_event(
            session,
            move |error| AppEvent::FollowingFailed {
                request_id,
                error: error.to_string(),
            },
            move |service, session| async move {
                let page = service
                    .load_followings(&session.tokens.access_token, next_href.as_deref())
                    .await?;
                Ok(AppEvent::FollowingLoaded {
                    session,
                    request_id,
                    page,
                    append,
                })
            },
        );
    }

    fn load_playlists(
        &self,
        session: AuthorizedSession,
        request_id: u64,
        next_href: Option<String>,
        append: bool,
    ) {
        self.spawn_session_event(
            session,
            move |error| AppEvent::PlaylistsFailed {
                request_id,
                error: error.to_string(),
            },
            move |service, session| async move {
                let page = service
                    .load_playlists(&session.tokens.access_token, next_href.as_deref())
                    .await?;
                Ok(AppEvent::PlaylistsLoaded {
                    session,
                    request_id,
                    page,
                    append,
                })
            },
        );
    }

    fn load_playlist_tracks(
        &self,
        session: AuthorizedSession,
        request_id: u64,
        playlist_urn: String,
        next_href: Option<String>,
        append: bool,
    ) {
        let failed_playlist_urn = playlist_urn.clone();
        self.spawn_session_event(
            session,
            move |error| AppEvent::PlaylistTracksFailed {
                request_id,
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
                    request_id,
                    playlist_urn,
                    page,
                    append,
                })
            },
        );
    }

    fn load_user_tracks(
        &self,
        session: AuthorizedSession,
        request_id: u64,
        user_urn: String,
        next_href: Option<String>,
        append: bool,
    ) {
        let failed_user_urn = user_urn.clone();
        self.spawn_session_event(
            session,
            move |error| AppEvent::UserTracksFailed {
                request_id,
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
                    request_id,
                    user_urn,
                    page,
                    append,
                })
            },
        );
    }

    fn load_user_playlists(
        &self,
        session: AuthorizedSession,
        request_id: u64,
        user_urn: String,
        next_href: Option<String>,
        append: bool,
    ) {
        let failed_user_urn = user_urn.clone();
        self.spawn_session_event(
            session,
            move |error| AppEvent::UserPlaylistsFailed {
                request_id,
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
                    request_id,
                    user_urn,
                    page,
                    append,
                })
            },
        );
    }

    fn search_all(&self, session: AuthorizedSession, request_id: u64, query: String) {
        let query_for_error = query.clone();
        self.spawn_session_event(
            session,
            move |error| AppEvent::SearchFailed {
                request_id,
                query: query_for_error,
                error: error.to_string(),
            },
            move |service, session| async move {
                let results = service
                    .search_all(&session.tokens.access_token, &query)
                    .await?;
                Ok(AppEvent::SearchLoaded {
                    session,
                    request_id,
                    query,
                    results,
                })
            },
        );
    }

    fn search_tracks_page(
        &self,
        session: AuthorizedSession,
        request_id: u64,
        query: String,
        next_href: String,
    ) {
        let query_for_error = query.clone();
        self.spawn_session_event(
            session,
            move |error| AppEvent::SearchTracksPageFailed {
                request_id,
                query: query_for_error,
                error: error.to_string(),
            },
            move |service, session| async move {
                let page = service
                    .search_tracks(&session.tokens.access_token, &query, Some(&next_href))
                    .await?;
                Ok(AppEvent::SearchTracksPageLoaded {
                    session,
                    request_id,
                    query,
                    page,
                })
            },
        );
    }

    fn like_track(&self, session: AuthorizedSession, track: TrackSummary) {
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

    fn add_track_to_playlist(
        &self,
        session: AuthorizedSession,
        track: TrackSummary,
        playlist: PlaylistSummary,
    ) {
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

    fn copy_text(&self, text: String, label: String) {
        let sender = self.sender.clone();
        task::spawn_blocking(move || {
            let event = match copy_text(&text) {
                Ok(()) => AppEvent::ClipboardCopied { label },
                Err(error) => AppEvent::ClipboardCopyFailed { label, error },
            };
            let _ = sender.send(event);
        });
    }

    fn load_cover_art(&self, url: String) {
        let http = self.service.http().clone();
        self.spawn_event_task(async move {
            let url_for_error = url.clone();
            match load_cover_art_event(http, url).await {
                Ok(event) => event,
                Err(error) => AppEvent::CoverArtFailed {
                    url: url_for_error,
                    error: error.to_string(),
                },
            }
        });
    }

    fn play_track(&self, session: AuthorizedSession, track: TrackSummary) {
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
                player.send(PlayerCommand::LoadTrack {
                    url: stream.url,
                    title: track.title.clone(),
                    authorization: Some(session.tokens.access_token.clone()),
                    duration_seconds: track.duration_ms.map(|duration| duration as f64 / 1000.0),
                })?;
                Ok(AppEvent::PlaybackQueued {
                    session,
                    title: track.title,
                    preview: stream.preview,
                })
            },
        );
    }

    fn control_playback(&self, command: PlayerCommand) {
        if let Err(error) = self.player.send(command) {
            let _ = self.sender.send(AppEvent::Player(PlayerEvent::BackendError(
                error.to_string(),
            )));
        }
    }

    fn control_visualizer(&self, command: VisualizerCommand) {
        if let Err(error) = self.visualizer.send(command) {
            let _ = self
                .sender
                .send(AppEvent::VisualizerCaptureFailed(error.to_string()));
        }
    }

    fn spawn_event_task<Fut>(&self, task: Fut)
    where
        Fut: Future<Output = AppEvent> + Send + 'static,
    {
        let sender = self.sender.clone();
        tokio::spawn(async move {
            let _ = sender.send(task.await);
        });
    }

    fn spawn_session_event<F, Fut, E>(&self, session: AuthorizedSession, on_error: E, run: F)
    where
        F: FnOnce(SoundcloudService, AuthorizedSession) -> Fut + Send + 'static,
        Fut: Future<Output = Result<AppEvent>> + Send + 'static,
        E: FnOnce(anyhow::Error) -> AppEvent + Send + 'static,
    {
        let sender = self.sender.clone();
        let paths = self.paths.clone();
        let service = self.service.clone();
        tokio::spawn(async move {
            let event = match session::execute(paths, service, session, run).await {
                Ok(event) => event,
                Err(error) => on_error(error),
            };
            let _ = sender.send(event);
        });
    }
}

fn format_error(error: &anyhow::Error) -> String {
    format!("{error:#}")
}

async fn load_cover_art_event(
    http: reqwest::Client,
    url: String,
) -> Result<AppEvent, reqwest::Error> {
    let response = http.get(&url).send().await?.error_for_status()?;
    let bytes = response.bytes().await?;
    Ok(AppEvent::CoverArtLoaded {
        url,
        bytes: bytes.to_vec(),
    })
}
