use super::*;
use crate::soundcloud::models::SearchResults;

impl AppState {
    pub fn maybe_queue_more_playlists(&mut self) -> bool {
        if self.session.is_none() || self.playlists.is_loading() {
            return false;
        }

        let Some(next_href) = self.playlists.next_href().map(str::to_string) else {
            return false;
        };
        let Some(session) = self.session.clone() else {
            return false;
        };

        let request_id = self.playlists.start_loading(true);
        self.queue_command(AppCommand::LoadPlaylists {
            session,
            request_id,
            next_href: Some(next_href),
            append: true,
        });
        self.status = "Loading more playlists...".to_string();
        true
    }

    pub fn maybe_queue_current_route_next_page(&mut self) -> bool {
        let Some(session) = self.session.clone() else {
            return false;
        };

        match self.route {
            Route::Feed => {
                if self.feed.loading {
                    return false;
                }
                let Some(next_href) = self.feed.next_href.clone() else {
                    return false;
                };
                self.feed.start_loading(true);
                let request_id = self.feed_request.issue(true);
                self.queue_command(AppCommand::LoadFeed {
                    session,
                    request_id,
                    next_href: Some(next_href),
                    append: true,
                });
                self.status = "Loading more feed items...".to_string();
                true
            }
            Route::LikedSongs => {
                if self.liked_tracks.loading {
                    return false;
                }
                let Some(next_href) = self.liked_tracks.next_href.clone() else {
                    return false;
                };
                self.liked_tracks.start_loading(true);
                let request_id = self.liked_tracks_request.issue(true);
                self.queue_command(AppCommand::LoadLikedSongs {
                    session,
                    request_id,
                    next_href: Some(next_href),
                    append: true,
                });
                self.status = "Loading more liked tracks...".to_string();
                true
            }
            Route::Albums => {
                if self.albums.loading {
                    return false;
                }
                let Some(next_href) = self.albums.next_href.clone() else {
                    return false;
                };
                self.albums.start_loading(true);
                let request_id = self.albums_request.issue(true);
                self.queue_command(AppCommand::LoadAlbums {
                    session,
                    request_id,
                    next_href: Some(next_href),
                    append: true,
                });
                self.status = "Loading more album-like playlists...".to_string();
                true
            }
            Route::Following => {
                if self.following.loading {
                    return false;
                }
                let Some(next_href) = self.following.next_href.clone() else {
                    return false;
                };
                self.following.start_loading(true);
                let request_id = self.following_request.issue(true);
                self.queue_command(AppCommand::LoadFollowing {
                    session,
                    request_id,
                    next_href: Some(next_href),
                    append: true,
                });
                self.status = "Loading more followed creators...".to_string();
                true
            }
            Route::Playlist => {
                let Some(urn) = self.active_playlist_urn.clone() else {
                    return false;
                };
                let next_href = {
                    let Some(state) = self.playlist_tracks.get_mut(&urn) else {
                        return false;
                    };
                    if state.loading {
                        return false;
                    }
                    let Some(next_href) = state.next_href.clone() else {
                        return false;
                    };
                    state.start_loading(true);
                    next_href
                };
                let request_id = self
                    .playlist_track_requests
                    .entry(urn.clone())
                    .or_default()
                    .issue(true);
                self.queue_command(AppCommand::LoadPlaylistTracks {
                    session,
                    request_id,
                    playlist_urn: urn,
                    next_href: Some(next_href),
                    append: true,
                });
                self.status = "Loading more playlist tracks...".to_string();
                true
            }
            Route::UserProfile => {
                let Some(user_urn) = self.active_user_profile_urn().map(str::to_string) else {
                    return false;
                };

                match self.user_profile_view {
                    UserProfileView::Tracks => {
                        if self.user_profile_tracks.loading {
                            return false;
                        }
                        let Some(next_href) = self.user_profile_tracks.next_href.clone() else {
                            return false;
                        };

                        self.user_profile_tracks.start_loading(true);
                        let request_id = self.user_profile_tracks_request.issue(true);
                        self.queue_command(AppCommand::LoadUserTracks {
                            session,
                            request_id,
                            user_urn,
                            next_href: Some(next_href),
                            append: true,
                        });
                        self.status = format!("Loading more tracks for {}...", self.route_title());
                        true
                    }
                    UserProfileView::Playlists => {
                        if self.user_profile_playlists.loading {
                            return false;
                        }
                        let Some(next_href) = self.user_profile_playlists.next_href.clone() else {
                            return false;
                        };

                        self.user_profile_playlists.start_loading(true);
                        let request_id = self.user_profile_playlists_request.issue(true);
                        self.queue_command(AppCommand::LoadUserPlaylists {
                            session,
                            request_id,
                            user_urn,
                            next_href: Some(next_href),
                            append: true,
                        });
                        self.status =
                            format!("Loading more playlists for {}...", self.route_title());
                        true
                    }
                }
            }
            Route::Search => {
                if self.search_view != SearchView::Tracks {
                    return false;
                }
                if self.search_tracks.loading {
                    return false;
                }
                let Some(next_href) = self.search_tracks.next_href.clone() else {
                    return false;
                };

                self.search_tracks.start_loading(true);
                let request_id = self.search_request.issue(true);
                self.queue_command(AppCommand::SearchTracksPage {
                    session,
                    request_id,
                    query: self.search_query.clone(),
                    next_href,
                });
                self.status = format!("Loading more search results for '{}'...", self.search_query);
                true
            }
            Route::RecentlyPlayed => false,
        }
    }
    pub(super) fn request_playlists_load(&mut self, append: bool) {
        let Some(session) = self.session.clone() else {
            return;
        };

        if self.playlists.is_loading() || (!append && self.playlists.is_loaded()) {
            return;
        }

        let request_id = self.playlists.start_loading(append);
        self.queue_command(AppCommand::LoadPlaylists {
            session,
            request_id,
            next_href: if append {
                self.playlists.next_href().map(str::to_string)
            } else {
                None
            },
            append,
        });
    }

    pub(super) fn request_route_load(&mut self, append: bool) {
        let Some(session) = self.session.clone() else {
            return;
        };

        match self.route {
            Route::Feed => {
                if self.feed.loading || (!append && self.feed.loaded) {
                    return;
                }
                self.feed.start_loading(append);
                let request_id = self.feed_request.issue(append);
                self.queue_command(AppCommand::LoadFeed {
                    session,
                    request_id,
                    next_href: if append {
                        self.feed.next_href.clone()
                    } else {
                        None
                    },
                    append,
                });
            }
            Route::LikedSongs => {
                if self.liked_tracks.loading || (!append && self.liked_tracks.loaded) {
                    return;
                }
                self.liked_tracks.start_loading(append);
                let request_id = self.liked_tracks_request.issue(append);
                self.queue_command(AppCommand::LoadLikedSongs {
                    session,
                    request_id,
                    next_href: if append {
                        self.liked_tracks.next_href.clone()
                    } else {
                        None
                    },
                    append,
                });
            }
            Route::RecentlyPlayed => {
                self.status = if self.recent_history.entries.is_empty() {
                    "Recently Played is empty until you finish a successful playback.".to_string()
                } else {
                    format!(
                        "Loaded {} locally stored plays.",
                        self.recent_history.entries.len()
                    )
                };
            }
            Route::Albums => {
                if self.albums.loading || (!append && self.albums.loaded) {
                    return;
                }
                self.albums.start_loading(append);
                let request_id = self.albums_request.issue(append);
                self.queue_command(AppCommand::LoadAlbums {
                    session,
                    request_id,
                    next_href: if append {
                        self.albums.next_href.clone()
                    } else {
                        None
                    },
                    append,
                });
            }
            Route::Following => {
                if self.following.loading || (!append && self.following.loaded) {
                    return;
                }
                self.following.start_loading(append);
                let request_id = self.following_request.issue(append);
                self.queue_command(AppCommand::LoadFollowing {
                    session,
                    request_id,
                    next_href: if append {
                        self.following.next_href.clone()
                    } else {
                        None
                    },
                    append,
                });
            }
            Route::Playlist => {
                let Some(urn) = self.active_playlist_urn.clone() else {
                    return;
                };
                let next_href = {
                    let state = self.playlist_tracks.entry(urn.clone()).or_default();
                    if state.loading || (!append && state.loaded) {
                        return;
                    }
                    let next_href = if append {
                        state.next_href.clone()
                    } else {
                        None
                    };
                    state.start_loading(append);
                    next_href
                };
                let request_id = self
                    .playlist_track_requests
                    .entry(urn.clone())
                    .or_default()
                    .issue(append);
                self.queue_command(AppCommand::LoadPlaylistTracks {
                    session,
                    request_id,
                    playlist_urn: urn,
                    next_href,
                    append,
                });
            }
            Route::UserProfile => {
                let Some(user_urn) = self.active_user_profile_urn().map(str::to_string) else {
                    self.status = "No creator profile is currently open.".to_string();
                    return;
                };

                match self.user_profile_view {
                    UserProfileView::Tracks => {
                        if self.user_profile_tracks.loading
                            || (!append && self.user_profile_tracks.loaded)
                        {
                            return;
                        }
                        self.user_profile_tracks.start_loading(append);
                        let request_id = self.user_profile_tracks_request.issue(append);
                        self.queue_command(AppCommand::LoadUserTracks {
                            session,
                            request_id,
                            user_urn,
                            next_href: if append {
                                self.user_profile_tracks.next_href.clone()
                            } else {
                                None
                            },
                            append,
                        });
                    }
                    UserProfileView::Playlists => {
                        if self.user_profile_playlists.loading
                            || (!append && self.user_profile_playlists.loaded)
                        {
                            return;
                        }
                        self.user_profile_playlists.start_loading(append);
                        let request_id = self.user_profile_playlists_request.issue(append);
                        self.queue_command(AppCommand::LoadUserPlaylists {
                            session,
                            request_id,
                            user_urn,
                            next_href: if append {
                                self.user_profile_playlists.next_href.clone()
                            } else {
                                None
                            },
                            append,
                        });
                    }
                }
            }
            Route::Search => {
                if self.search_query.trim().is_empty() {
                    self.status = "Enter a search query first.".to_string();
                    return;
                }
                if append {
                    self.maybe_queue_current_route_next_page();
                    return;
                }
                if !append {
                    if let Some(cache) = self.search_cache.get(&self.search_query).cloned() {
                        self.search_tracks = cache.tracks;
                        self.search_playlists = cache.playlists;
                        self.search_users = cache.users;
                        self.status =
                            format!("Loaded cached search results for '{}'.", self.search_query);
                        return;
                    }
                }
                self.search_tracks.start_loading(false);
                self.search_playlists.start_loading(false);
                self.search_users.start_loading(false);
                let request_id = self.search_request.issue(false);
                self.queue_command(AppCommand::SearchAll {
                    session,
                    request_id,
                    query: self.search_query.clone(),
                });
            }
        }
    }

    pub(super) fn invalidate_liked_tracks(&mut self) {
        self.liked_tracks = CollectionState::default();
        self.liked_tracks_request.invalidate();
        if self.route == Route::LikedSongs {
            self.request_route_load(false);
        }
    }

    pub(super) fn invalidate_playlists_sidebar(&mut self) {
        self.playlists.invalidate();
        self.request_playlists_load(false);
    }

    pub(super) fn invalidate_playlist_tracks(&mut self, playlist_urn: &str) {
        self.playlist_tracks
            .insert(playlist_urn.to_string(), CollectionState::default());
        self.playlist_track_requests
            .entry(playlist_urn.to_string())
            .or_default()
            .invalidate();

        if self.active_playlist_urn.as_deref() == Some(playlist_urn)
            && self.route == Route::Playlist
        {
            self.request_route_load(false);
        }
    }

    pub(super) fn bump_playlist_track_count(&mut self, playlist_urn: &str) {
        if let Some(playlist) = self.known_playlists.get_mut(playlist_urn) {
            playlist.track_count = playlist.track_count.saturating_add(1);
        }
    }

    pub(super) fn reset_live_data(&mut self) {
        self.playlists.reset();
        self.active_playlist_urn = None;
        self.known_playlists.clear();
        self.feed = CollectionState::default();
        self.feed_request = RequestTracker::default();
        self.liked_tracks = CollectionState::default();
        self.liked_tracks_request = RequestTracker::default();
        self.albums = CollectionState::default();
        self.albums_request = RequestTracker::default();
        self.following = CollectionState::default();
        self.following_request = RequestTracker::default();
        self.playlist_tracks.clear();
        self.playlist_track_requests.clear();
        self.search_tracks = CollectionState::default();
        self.search_playlists = CollectionState::default();
        self.search_users = CollectionState::default();
        self.search_request = RequestTracker::default();
        self.search_view = SearchView::Tracks;
        self.active_user_profile = None;
        self.user_profile_tracks = CollectionState::default();
        self.user_profile_tracks_request = RequestTracker::default();
        self.user_profile_playlists = CollectionState::default();
        self.user_profile_playlists_request = RequestTracker::default();
        self.user_profile_view = UserProfileView::Tracks;
        self.search_cache.clear();
        self.selected_playlist = 0;
        self.selected_content = 0;
        self.add_to_playlist_modal = None;
        self.logout_confirm_modal = None;
        self.queue = QueueState::default();
        self.playback_plan = PlaybackPlanState::default();
        self.player = PlayerState {
            status: PlaybackStatus::Stopped,
            volume_percent: self.settings.volume_percent as f64,
            position_seconds: 0.0,
            duration_seconds: None,
            shuffle_enabled: false,
            repeat_mode: RepeatMode::Off,
        };
        self.now_playing.track = None;
        self.now_playing.title = "Nothing playing".to_string();
        self.now_playing.artist = "Select a track and press Enter".to_string();
        self.now_playing.context = "Idle".to_string();
        self.now_playing.artwork_url = None;
        self.now_playing.progress_ratio = 0.0;
        self.now_playing.elapsed_label = "0:00".to_string();
        self.now_playing.duration_label = "0:00".to_string();
        self.cover_art = CoverArt::default();
    }

    pub(super) fn apply_playlists_page(&mut self, page: Page<SoundcloudPlaylist>, append: bool) {
        let mapped = page
            .items
            .into_iter()
            .map(|playlist| {
                self.remember_playlist(playlist.clone());
                SidebarPlaylist {
                    urn: Some(playlist.urn),
                    title: playlist.title,
                    description: playlist.description,
                    creator: Some(playlist.creator),
                    track_count: Some(playlist.track_count),
                    tracks: Vec::new(),
                }
            })
            .collect::<Vec<_>>();

        self.playlists.apply_page(
            Page {
                items: mapped,
                next_href: page.next_href,
            },
            append,
        );

        if self.playlists.is_empty() {
            self.selected_playlist = 0;
            if self.route == Route::Playlist {
                self.route = Route::Feed;
            }
        } else {
            self.selected_playlist = self.selected_playlist.min(self.playlists.len() - 1);
        }

        self.status = format!("Loaded {} playlists.", self.playlists.len());
    }

    pub(super) fn apply_search_results(&mut self, results: SearchResults) {
        self.search_tracks.apply_page(results.tracks, false);
        for playlist in &results.playlists.items {
            self.remember_playlist(playlist.clone());
        }
        self.search_playlists.apply_page(results.playlists, false);
        self.search_users.apply_page(results.users, false);
    }

    pub(super) fn reload_current_route(&mut self) {
        if self.focus == Focus::Playlists {
            self.playlists.reset();
            self.status = "Reloading playlists...".to_string();
            self.request_playlists_load(false);
            return;
        }

        match self.route {
            Route::Feed => self.feed = CollectionState::default(),
            Route::LikedSongs => self.liked_tracks = CollectionState::default(),
            Route::RecentlyPlayed => {
                self.status = "Recently Played is local and already up to date.".to_string();
                return;
            }
            Route::Albums => self.albums = CollectionState::default(),
            Route::Following => self.following = CollectionState::default(),
            Route::Playlist => {
                let Some(urn) = self.active_playlist_urn.clone() else {
                    self.status = "No playlist is currently open.".to_string();
                    return;
                };
                self.playlist_tracks.insert(urn, CollectionState::default());
            }
            Route::UserProfile => {
                if self.active_user_profile.is_none() {
                    self.status = "No creator profile is currently open.".to_string();
                    return;
                }
                self.user_profile_tracks = CollectionState::default();
                self.user_profile_playlists = CollectionState::default();
            }
            Route::Search => {
                self.search_cache.remove(&self.search_query);
                self.search_tracks = CollectionState::default();
                self.search_playlists = CollectionState::default();
                self.search_users = CollectionState::default();
            }
        }

        if self.route == Route::Search && self.search_query.trim().is_empty() {
            self.status = "Enter a search query first.".to_string();
            return;
        }

        self.status = format!("Reloading {}...", self.route_title());
        self.request_route_load(false);
    }
}
