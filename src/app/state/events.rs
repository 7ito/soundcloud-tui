impl AppState {
    pub fn dispatch_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Key(key) => self.handle_key_event(key),
            AppEvent::Mouse(mouse) => self.handle_mouse_event(mouse),
            AppEvent::Paste(text) => self.handle_paste_event(&text),
            AppEvent::Tick => self.on_tick(),
            AppEvent::Resize { width, height } => self.on_resize(width, height),
            AppEvent::CredentialsSaved(request) => {
                self.auth.set_waiting_for_browser(request.clone());
                self.loading = None;
                self.status =
                    "Saved credentials locally. Authorize the app in your browser.".to_string();
                self.queue_command(AppCommand::OpenUrl(request.authorize_url.clone()));
                self.queue_command(AppCommand::WaitForOAuthCallback(request));
            }
            AppEvent::CredentialsSaveFailed(error) => {
                let message = format!("Could not save credentials locally: {error}");
                self.auth.set_error(message.clone());
                self.loading = None;
                self.status = message;
            }
            AppEvent::AuthRestoreComplete(result) => match result {
                Ok(session) => self.complete_auth(session),
                Err(error) => {
                    self.mode = AppMode::Auth;
                    self.auth.step = crate::app::AuthStep::Credentials;
                    self.auth.set_error(error.clone());
                    self.auth
                        .set_info("No reusable session was found. Continue with browser login.");
                    self.loading = None;
                    self.status = error;
                }
            },
            AppEvent::AuthCallbackCaptured(callback_input) => {
                if let Some(request) = self.auth.pending_authorization.clone() {
                    self.auth.clear_error();
                    self.auth
                        .set_info("Received a callback. Exchanging the authorization code...");
                    self.set_loading("Exchanging SoundCloud authorization code...");
                    self.queue_command(AppCommand::ExchangeAuthorizationCode {
                        request,
                        callback_input,
                    });
                }
            }
            AppEvent::AuthCallbackFailed(error) => {
                self.auth.set_error(error.clone());
                self.auth.show_manual_callback(
                    "Paste the full redirected callback URL from your browser to continue.",
                );
                self.loading = None;
                self.status = error;
            }
            AppEvent::AuthCompleted(result) => match result {
                Ok(session) => self.complete_auth(session),
                Err(error) => {
                    self.auth.set_error(error.clone());
                    self.auth.show_manual_callback(
                        "The callback was captured, but the token exchange failed. Check the URL and try again.",
                    );
                    self.loading = None;
                    self.status = error;
                }
            },
            AppEvent::FeedLoaded {
                session,
                page,
                append,
            } => {
                self.session = Some(session);
                for item in &page.items {
                    if let FeedOrigin::Playlist(playlist) = &item.origin {
                        self.remember_playlist(playlist.clone());
                    }
                }
                self.feed.apply_page(page, append);
                self.status = format!("Loaded {} feed items.", self.feed.items.len());
            }
            AppEvent::FeedFailed(error) => {
                self.feed.fail(error.clone());
                self.show_main_error("Could not load feed", error);
            }
            AppEvent::LikedSongsLoaded {
                session,
                page,
                append,
            } => {
                self.session = Some(session);
                self.liked_tracks.apply_page(page, append);
                self.status = format!("Loaded {} liked tracks.", self.liked_tracks.items.len());
            }
            AppEvent::LikedSongsFailed(error) => {
                self.liked_tracks.fail(error.clone());
                self.show_main_error("Could not load liked songs", error);
            }
            AppEvent::AlbumsLoaded {
                session,
                page,
                append,
            } => {
                self.session = Some(session);
                for playlist in &page.items {
                    self.remember_playlist(playlist.clone());
                }
                self.albums.apply_page(page, append);
                self.status = format!("Loaded {} album-like playlists.", self.albums.items.len());
            }
            AppEvent::AlbumsFailed(error) => {
                self.albums.fail(error.clone());
                self.show_main_error("Could not load albums", error);
            }
            AppEvent::FollowingLoaded {
                session,
                page,
                append,
            } => {
                self.session = Some(session);
                self.following.apply_page(page, append);
                self.status = format!("Loaded {} followed creators.", self.following.items.len());
            }
            AppEvent::FollowingFailed(error) => {
                self.following.fail(error.clone());
                self.show_main_error("Could not load followed creators", error);
            }
            AppEvent::PlaylistsLoaded {
                session,
                page,
                append,
            } => {
                self.session = Some(session);
                self.apply_playlists_page(page, append);
            }
            AppEvent::PlaylistsFailed(error) => {
                self.playlists_loading = false;
                self.playlists_loaded = true;
                self.playlists_error = Some(error.clone());
                self.show_main_error("Could not load playlists", error);
            }
            AppEvent::PlaylistTracksLoaded {
                session,
                playlist_urn,
                page,
                append,
            } => {
                self.session = Some(session);
                let state = self.playlist_tracks.entry(playlist_urn).or_default();
                state.apply_page(page, append);
                self.status = format!("Loaded {} playlist tracks.", state.items.len());
            }
            AppEvent::PlaylistTracksFailed {
                playlist_urn,
                error,
            } => {
                self.playlist_tracks
                    .entry(playlist_urn)
                    .or_default()
                    .fail(error.clone());
                self.show_main_error("Could not load playlist tracks", error);
            }
            AppEvent::UserTracksLoaded {
                session,
                user_urn,
                page,
                append,
            } => {
                if self.active_user_profile_urn() != Some(user_urn.as_str()) {
                    return;
                }

                self.session = Some(session);
                self.user_profile_tracks.apply_page(page, append);
                self.status = format!(
                    "Loaded {} tracks for {}.",
                    self.user_profile_tracks.items.len(),
                    self.route_title()
                );
            }
            AppEvent::UserTracksFailed { user_urn, error } => {
                if self.active_user_profile_urn() != Some(user_urn.as_str()) {
                    return;
                }

                self.user_profile_tracks.fail(error.clone());
                self.show_main_error("Could not load user tracks", error);
            }
            AppEvent::UserPlaylistsLoaded {
                session,
                user_urn,
                page,
                append,
            } => {
                if self.active_user_profile_urn() != Some(user_urn.as_str()) {
                    return;
                }

                self.session = Some(session);
                for playlist in &page.items {
                    self.remember_playlist(playlist.clone());
                }
                self.user_profile_playlists.apply_page(page, append);
                self.status = format!(
                    "Loaded {} playlists for {}.",
                    self.user_profile_playlists.items.len(),
                    self.route_title()
                );
            }
            AppEvent::UserPlaylistsFailed { user_urn, error } => {
                if self.active_user_profile_urn() != Some(user_urn.as_str()) {
                    return;
                }

                self.user_profile_playlists.fail(error.clone());
                self.show_main_error("Could not load user playlists", error);
            }
            AppEvent::SearchLoaded {
                session,
                query,
                results,
            } => {
                if query != self.search_query {
                    return;
                }

                self.session = Some(session);
                self.apply_search_results(results);
                self.cache_search_results();
                self.status = format!(
                    "Search ready: {} tracks, {} playlists, {} users.",
                    self.search_tracks.items.len(),
                    self.search_playlists.items.len(),
                    self.search_users.items.len(),
                );
            }
            AppEvent::SearchFailed { query, error } => {
                if query != self.search_query {
                    return;
                }

                self.search_tracks.fail(error.clone());
                self.search_playlists.fail(error.clone());
                self.search_users.fail(error.clone());
                self.show_main_error("Could not load search results", error);
            }
            AppEvent::SearchTracksPageLoaded {
                session,
                query,
                page,
            } => {
                if query != self.search_query {
                    return;
                }

                self.session = Some(session);
                self.search_tracks.apply_page(page, true);
                self.cache_search_results();
                self.status = format!(
                    "Loaded {} track search results.",
                    self.search_tracks.items.len()
                );
            }
            AppEvent::SearchTracksPageFailed { query, error } => {
                if query != self.search_query {
                    return;
                }

                self.search_tracks.fail(error.clone());
                self.show_main_error("Could not load more search results", error);
            }
            AppEvent::TrackLiked {
                session,
                track_title,
            } => {
                self.session = Some(session);
                self.invalidate_liked_tracks();
                self.status = format!("Added {track_title} to Liked Songs.");
                self.show_toast("Added to Liked Songs");
            }
            AppEvent::TrackLikeFailed { track_title, error } => {
                self.show_main_error(
                    "Could not add track to Liked Songs",
                    format!("Could not add {track_title} to Liked Songs.\n\n{error}"),
                );
            }
            AppEvent::TrackAddedToPlaylist {
                session,
                playlist_urn,
                playlist_title,
                track_title,
                already_present,
            } => {
                self.session = Some(session);
                self.add_to_playlist_modal = None;
                self.invalidate_playlists_sidebar();
                self.invalidate_playlist_tracks(&playlist_urn);
                if already_present {
                    self.status = format!("{track_title} is already in {playlist_title}.");
                    self.show_toast("Already in playlist");
                } else {
                    self.bump_playlist_track_count(&playlist_urn);
                    self.status = format!("Added {track_title} to {playlist_title}.");
                    self.show_toast("Added to playlist");
                }
            }
            AppEvent::TrackAddToPlaylistFailed {
                playlist_title,
                track_title,
                error,
            } => {
                self.add_to_playlist_modal = None;
                self.show_main_error(
                    "Could not add track to playlist",
                    format!("Could not add {track_title} to {playlist_title}.\n\n{error}"),
                );
            }
            AppEvent::ClipboardCopied { label } => {
                self.status = format!("Copied {} URL to the clipboard.", label);
                self.show_toast("Copied URL to clipboard");
            }
            AppEvent::ClipboardCopyFailed { label, error } => {
                self.show_main_error(
                    "Could not copy share URL",
                    format!("Could not copy the SoundCloud URL for {label}.\n\n{error}"),
                );
            }
            AppEvent::CoverArtLoaded { url, bytes } => {
                if self.cover_art.url.as_deref() != Some(url.as_str()) {
                    return;
                }

                self.cover_art.bytes = Some(bytes);
                self.cover_art.loading = false;
            }
            AppEvent::CoverArtFailed { url, error } => {
                if self.cover_art.url.as_deref() != Some(url.as_str()) {
                    return;
                }

                self.cover_art.bytes = None;
                self.cover_art.loading = false;
                self.status = format!("Could not load cover art: {error}");
            }
            AppEvent::PlaybackQueued {
                session,
                title,
                preview,
            } => {
                self.session = Some(session);
                self.player.status = PlaybackStatus::Buffering;
                self.status = if preview {
                    format!("Streaming preview for {title}.")
                } else {
                    format!("Starting playback for {title}.")
                };
            }
            AppEvent::PlaybackFailed { title, error } => {
                self.player.status = PlaybackStatus::Stopped;
                self.show_main_error(
                    "Could not start playback",
                    format!("Could not start playback for {title}.\n\n{error}"),
                );
            }
            AppEvent::VisualizerFrame(frame) => {
                if !self.visualizer.visible {
                    return;
                }

                self.visualizer.capture_active = true;
                self.visualizer.spectrum = frame;
            }
            AppEvent::VisualizerCaptureStarted(message) => {
                if !self.visualizer.visible {
                    return;
                }

                self.visualizer.capture_active = true;
                self.visualizer.status = message.clone();
                self.status = message;
            }
            AppEvent::VisualizerCaptureFailed(error) => {
                if !self.visualizer.visible {
                    return;
                }

                self.visualizer.capture_active = false;
                self.visualizer.status = error.clone();
                self.status = error;
            }
            AppEvent::PlaybackIntent(intent) => self.apply_playback_intent(intent),
            AppEvent::Player(event) => self.apply_player_event(event),
        }
    }

    pub fn set_auth_session(&mut self, session: &AuthorizedSession) {
        self.auth_summary = match &session.profile.permalink_url {
            Some(permalink) => {
                format!(
                    "Authenticated as {} ({})",
                    session.profile.username, permalink
                )
            }
            None => format!("Authenticated as {}", session.profile.username),
        };
        self.status = format!("Ready. {}", self.auth_summary);
    }

    pub fn begin_saved_session_validation(&mut self, credentials: Credentials, tokens: TokenStore) {
        self.mode = AppMode::Auth;
        self.auth.set_checking_session();
        self.set_loading("Checking existing SoundCloud session...");
        self.queue_command(AppCommand::ValidateSavedSession {
            credentials,
            tokens,
        });
    }

    pub fn take_pending_command(&mut self) -> Option<AppCommand> {
        if self.pending_commands.is_empty() {
            None
        } else {
            self.pending_commands.pop_front()
        }
    }
}
