use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{
    app::{
        reducer, Action, AppCommand, AppEvent, AppMode, AuthIntent, AuthState, Focus,
        PlaybackIntent, RepeatMode, Route,
    },
    config::{credentials::Credentials, tokens::TokenStore},
    input::events::{is_global_quit_key, map_main_key_event},
    player::{command::PlayerCommand, event::PlayerEvent},
    soundcloud::{
        auth::AuthorizedSession,
        models::{
            FeedItem, FeedOrigin, PlaylistSummary as SoundcloudPlaylist, SearchResults,
            TrackSummary, UserSummary,
        },
        paging::Page,
    },
};

#[derive(Debug, Clone)]
pub struct AppState {
    pub mode: AppMode,
    pub route: Route,
    pub focus: Focus,
    pub should_quit: bool,
    pub auth: AuthState,
    pub session: Option<AuthorizedSession>,
    pub auth_summary: String,
    pub status: String,
    pub tick_count: u64,
    pub viewport: Viewport,
    pub loading: Option<LoadingState>,
    pub search_query: String,
    pub search_cursor: usize,
    pub library_items: Vec<LibraryItem>,
    pub playlists: Vec<SidebarPlaylist>,
    pub feed_rows: Vec<ContentRow>,
    pub liked_rows: Vec<ContentRow>,
    pub recent_rows: Vec<ContentRow>,
    pub album_rows: Vec<ContentRow>,
    pub following_rows: Vec<ContentRow>,
    pub search_rows: Vec<ContentRow>,
    pub selected_library: usize,
    pub selected_playlist: usize,
    pub selected_content: usize,
    pub now_playing: NowPlaying,
    pub player: PlayerState,
    pub queue: QueueState,
    pending_commands: Vec<AppCommand>,
    feed: CollectionState<FeedItem>,
    liked_tracks: CollectionState<TrackSummary>,
    albums: CollectionState<SoundcloudPlaylist>,
    following: CollectionState<UserSummary>,
    playlist_tracks: HashMap<String, CollectionState<TrackSummary>>,
    search_tracks: CollectionState<TrackSummary>,
    search_playlists: CollectionState<SoundcloudPlaylist>,
    search_users: CollectionState<UserSummary>,
    playlists_loading: bool,
    playlists_loaded: bool,
    playlists_error: Option<String>,
    playlists_next_href: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LibraryItem {
    pub label: &'static str,
    pub route: Route,
}

#[derive(Debug, Clone)]
pub struct SidebarPlaylist {
    pub urn: Option<String>,
    pub title: String,
    pub description: String,
    pub creator: Option<String>,
    pub track_count: Option<usize>,
    pub tracks: Vec<ContentRow>,
}

#[derive(Debug, Clone)]
pub struct ContentRow {
    pub columns: [String; 4],
}

#[derive(Debug, Clone)]
pub struct NowPlaying {
    pub track: Option<TrackSummary>,
    pub title: String,
    pub artist: String,
    pub context: String,
    pub elapsed_label: String,
    pub duration_label: String,
    pub progress_ratio: f64,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PlaybackStatus {
    Stopped,
    Buffering,
    Playing,
    Paused,
}

impl PlaybackStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Stopped => "Stopped",
            Self::Buffering => "Buffering",
            Self::Playing => "Playing",
            Self::Paused => "Paused",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlayerState {
    pub status: PlaybackStatus,
    pub volume_percent: f64,
    pub position_seconds: f64,
    pub duration_seconds: Option<f64>,
    pub shuffle_enabled: bool,
    pub repeat_mode: RepeatMode,
}

#[derive(Debug, Clone, Default)]
pub struct QueueState {
    pub tracks: Vec<TrackSummary>,
    pub current_index: Option<usize>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Viewport {
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LoadingState {
    pub message: String,
    pub ticks_remaining: u8,
}

#[derive(Debug, Clone)]
pub struct ContentView {
    pub title: String,
    pub subtitle: String,
    pub columns: [&'static str; 4],
    pub rows: Vec<ContentRow>,
    pub state_label: String,
    pub empty_message: String,
}

#[derive(Debug, Clone)]
struct CollectionState<T> {
    items: Vec<T>,
    next_href: Option<String>,
    loading: bool,
    error: Option<String>,
    loaded: bool,
}

#[derive(Debug, Clone)]
enum SelectedContent {
    Track {
        track: TrackSummary,
        context: String,
    },
    Playlist(SoundcloudPlaylist),
    User(UserSummary),
}

impl<T> Default for CollectionState<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            next_href: None,
            loading: false,
            error: None,
            loaded: false,
        }
    }
}

impl<T> CollectionState<T> {
    fn start_loading(&mut self, append: bool) {
        self.loading = true;
        self.error = None;
        if !append {
            self.next_href = None;
            self.items.clear();
        }
    }

    fn apply_page(&mut self, page: Page<T>, append: bool) {
        self.loading = false;
        self.error = None;
        self.loaded = true;
        self.next_href = page.next_href;
        if append {
            self.items.extend(page.items);
        } else {
            self.items = page.items;
        }
    }

    fn fail(&mut self, error: String) {
        self.loading = false;
        self.error = Some(error);
        self.loaded = true;
    }

    fn state_label(&self) -> String {
        if self.loading {
            "Loading".to_string()
        } else if let Some(error) = &self.error {
            format!("Error: {error}")
        } else if self.loaded {
            if self.items.is_empty() {
                "Empty".to_string()
            } else if self.next_href.is_some() {
                format!("Loaded {} items (more available)", self.items.len())
            } else {
                format!("Loaded {} items", self.items.len())
            }
        } else {
            "Waiting".to_string()
        }
    }
}

impl AppState {
    pub fn new() -> Self {
        let library_items = vec![
            LibraryItem {
                label: "Feed",
                route: Route::Feed,
            },
            LibraryItem {
                label: "Liked Songs",
                route: Route::LikedSongs,
            },
            LibraryItem {
                label: "Recently Played",
                route: Route::RecentlyPlayed,
            },
            LibraryItem {
                label: "Albums",
                route: Route::Albums,
            },
            LibraryItem {
                label: "Following",
                route: Route::Following,
            },
        ];

        let playlists = mock_playlists();
        let feed_rows = mock_track_rows(&[
            ("Canvas", "Tourist", "From Nils Frahm repost", "4:11"),
            ("Pillar", "Bonobo", "Friends upload", "3:54"),
            ("Tied", "Ross From Friends", "Weekend rotation", "5:26"),
            ("Float", "TSHA", "Daily feed", "4:07"),
            ("Luna Park", "Logic1000", "Club notes", "3:43"),
        ]);
        let liked_rows = mock_track_rows(&[
            ("Whisper", "Fred again..", "Liked Songs", "3:39"),
            ("Mosaic", "Overmono", "Liked Songs", "4:23"),
            ("Blue Drift", "Dj Seinfeld", "Liked Songs", "5:01"),
            ("Signal", "Four Tet", "Liked Songs", "4:34"),
            ("Tides", "Caribou", "Liked Songs", "4:56"),
        ]);
        let recent_rows = mock_track_rows(&[
            ("Heatmap", "Bicep", "Yesterday afternoon", "4:20"),
            ("Harbor", "Tycho", "Morning focus", "5:05"),
            ("Bodyline", "Jamie xx", "Late-night coding", "3:58"),
            ("Avenue", "Kelly Lee Owens", "Commute", "4:14"),
            ("Driftglass", "George Fitzgerald", "Coffee break", "4:33"),
        ]);
        let album_rows = mock_track_rows(&[
            ("Fragments", "Bonobo", "12 tracks", "2022"),
            ("Singularity", "Jon Hopkins", "10 tracks", "2018"),
            ("Immunity", "Jon Hopkins", "8 tracks", "2013"),
            ("Awake", "Tycho", "8 tracks", "2014"),
        ]);
        let following_rows = mock_track_rows(&[
            ("Ross From Friends", "184K", "New EP teaser", "Online"),
            ("TSHA", "221K", "Tour diary", "Online"),
            ("Tourist", "129K", "Studio notes", "Away"),
            ("Ford.", "96K", "Ambient session", "Online"),
        ]);
        let search_rows = mock_track_rows(&[
            ("Sketch One", "Model Man", "Search result", "3:17"),
            ("Sketch Two", "Daniel Avery", "Search result", "4:42"),
            ("Sketch Three", "Kettama", "Search result", "3:56"),
            ("Sketch Four", "Brijean", "Search result", "4:18"),
            ("Sketch Five", "Maribou State", "Search result", "5:09"),
        ]);

        Self {
            mode: AppMode::Main,
            route: Route::Feed,
            focus: Focus::Library,
            should_quit: false,
            auth: AuthState::new(Credentials::default()),
            session: None,
            auth_summary: "Unauthenticated".to_string(),
            status: "Tab cycles panes, arrows move, Enter selects, q quits.".to_string(),
            tick_count: 0,
            viewport: Viewport {
                width: 0,
                height: 0,
            },
            loading: Some(LoadingState {
                message: "Preparing mock library data...".to_string(),
                ticks_remaining: 2,
            }),
            search_query: "deep house sketches".to_string(),
            search_cursor: "deep house sketches".chars().count(),
            library_items,
            playlists,
            feed_rows,
            liked_rows,
            recent_rows,
            album_rows,
            following_rows,
            search_rows,
            selected_library: 0,
            selected_playlist: 0,
            selected_content: 0,
            now_playing: NowPlaying {
                track: None,
                title: "Phase 1 shell".to_string(),
                artist: "No track selected".to_string(),
                context: "Select a row in the content pane".to_string(),
                elapsed_label: "0:00".to_string(),
                duration_label: "0:00".to_string(),
                progress_ratio: 0.0,
            },
            player: PlayerState {
                status: PlaybackStatus::Stopped,
                volume_percent: 50.0,
                position_seconds: 0.0,
                duration_seconds: None,
                shuffle_enabled: false,
                repeat_mode: RepeatMode::Off,
            },
            queue: QueueState::default(),
            pending_commands: Vec::new(),
            feed: CollectionState::default(),
            liked_tracks: CollectionState::default(),
            albums: CollectionState::default(),
            following: CollectionState::default(),
            playlist_tracks: HashMap::new(),
            search_tracks: CollectionState::default(),
            search_playlists: CollectionState::default(),
            search_users: CollectionState::default(),
            playlists_loading: false,
            playlists_loaded: false,
            playlists_error: None,
            playlists_next_href: None,
        }
    }

    pub fn new_onboarding(credentials: Credentials) -> Self {
        let mut app = Self::new();
        app.mode = AppMode::Auth;
        app.auth = AuthState::new(credentials);
        app.auth_summary = "Not authenticated yet".to_string();
        app.loading = None;
        app.status = "Finish SoundCloud setup to enter the main player shell.".to_string();
        app
    }

    pub fn apply(&mut self, action: Action) {
        reducer::reduce(self, action);
    }

    pub fn dispatch_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Key(key) => self.handle_key_event(key),
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
                self.feed.apply_page(page, append);
                self.status = format!("Loaded {} feed items.", self.feed.items.len());
            }
            AppEvent::FeedFailed(error) => {
                self.feed.fail(error.clone());
                self.status = error;
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
                self.status = error;
            }
            AppEvent::AlbumsLoaded {
                session,
                page,
                append,
            } => {
                self.session = Some(session);
                self.albums.apply_page(page, append);
                self.status = format!("Loaded {} album-like playlists.", self.albums.items.len());
            }
            AppEvent::AlbumsFailed(error) => {
                self.albums.fail(error.clone());
                self.status = error;
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
                self.status = error;
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
                self.status = error;
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
                self.status = error;
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
                self.status = error;
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
                self.status = error;
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
                self.status = format!("Could not start playback for {title}: {error}");
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
            Some(self.pending_commands.remove(0))
        }
    }

    pub fn current_content(&self) -> ContentView {
        if self.session.is_none() {
            return self.mock_content();
        }

        match self.route {
            Route::Feed => ContentView {
                title: "Feed".to_string(),
                subtitle: "Recent track activity from the accounts you follow.".to_string(),
                columns: ["Title", "Artist", "Source", "Length"],
                rows: self
                    .feed
                    .items
                    .iter()
                    .map(|item| ContentRow {
                        columns: match &item.origin {
                            FeedOrigin::Track(track) => [
                                track.title.clone(),
                                track.artist.clone(),
                                pretty_activity_type(&item.activity_type),
                                track.duration_label(),
                            ],
                            FeedOrigin::Playlist(playlist) => [
                                playlist.title.clone(),
                                playlist.creator.clone(),
                                pretty_activity_type(&item.activity_type),
                                playlist.track_count_label(),
                            ],
                        },
                    })
                    .collect(),
                state_label: self.feed.state_label(),
                empty_message: "No feed activity is available right now.".to_string(),
            },
            Route::LikedSongs => ContentView {
                title: "Liked Songs".to_string(),
                subtitle: "Tracks you have liked on SoundCloud.".to_string(),
                columns: ["Title", "Artist", "Access", "Length"],
                rows: self
                    .liked_tracks
                    .items
                    .iter()
                    .map(track_row_with_access)
                    .collect(),
                state_label: self.liked_tracks.state_label(),
                empty_message: "No liked tracks were returned by SoundCloud.".to_string(),
            },
            Route::RecentlyPlayed => ContentView {
                title: "Recently Played".to_string(),
                subtitle: "Local history remains a later phase.".to_string(),
                columns: ["Title", "Artist", "Last Context", "Length"],
                rows: self.recent_rows.clone(),
                state_label: "Local placeholder".to_string(),
                empty_message: "Recently played is not wired yet.".to_string(),
            },
            Route::Albums => ContentView {
                title: "Albums".to_string(),
                subtitle: "Album-like playlists derived from your SoundCloud library.".to_string(),
                columns: ["Album", "Creator", "Tracks", "Year"],
                rows: self
                    .albums
                    .items
                    .iter()
                    .map(|playlist| ContentRow {
                        columns: [
                            playlist.title.clone(),
                            playlist.creator.clone(),
                            playlist.track_count_label(),
                            playlist.year_label(),
                        ],
                    })
                    .collect(),
                state_label: self.albums.state_label(),
                empty_message: "No album-like playlists were found.".to_string(),
            },
            Route::Following => ContentView {
                title: "Following".to_string(),
                subtitle: "Accounts you follow on SoundCloud.".to_string(),
                columns: ["Creator", "Followers", "Catalog", "Profile"],
                rows: self
                    .following
                    .items
                    .iter()
                    .map(|user| ContentRow {
                        columns: [
                            user.username.clone(),
                            user.followers_label(),
                            user.spotlight_label(),
                            if user.permalink_url.is_some() {
                                "Profile".to_string()
                            } else {
                                "--".to_string()
                            },
                        ],
                    })
                    .collect(),
                state_label: self.following.state_label(),
                empty_message: "This account is not following anyone yet.".to_string(),
            },
            Route::Playlist(index) => {
                let playlist = self.playlists.get(index);
                let tracks = playlist
                    .and_then(|playlist| playlist.urn.as_ref())
                    .and_then(|urn| self.playlist_tracks.get(urn));

                ContentView {
                    title: playlist
                        .map(|playlist| playlist.title.clone())
                        .unwrap_or_else(|| "Playlist".to_string()),
                    subtitle: playlist
                        .map(sidebar_playlist_subtitle)
                        .unwrap_or_else(|| "Playlist details are loading.".to_string()),
                    columns: ["Title", "Artist", "Access", "Length"],
                    rows: tracks
                        .map(|state| state.items.iter().map(track_row_with_access).collect())
                        .unwrap_or_default(),
                    state_label: tracks
                        .map(CollectionState::state_label)
                        .unwrap_or_else(|| "Waiting".to_string()),
                    empty_message: "No tracks are available for this playlist.".to_string(),
                }
            }
            Route::Search => ContentView {
                title: format!("Search: {}", self.search_query),
                subtitle: format!(
                    "Track-first results. Also found {} playlists and {} users.",
                    self.search_playlists.items.len(),
                    self.search_users.items.len(),
                ),
                columns: ["Title", "Artist", "Access", "Length"],
                rows: self
                    .search_tracks
                    .items
                    .iter()
                    .map(track_row_with_access)
                    .collect(),
                state_label: self.search_tracks.state_label(),
                empty_message: "No matching tracks were found.".to_string(),
            },
        }
    }

    pub fn current_content_len(&self) -> usize {
        self.current_content().rows.len()
    }

    pub fn current_content_row(&self) -> Option<ContentRow> {
        self.current_content()
            .rows
            .get(self.selected_content)
            .cloned()
    }

    pub fn current_selection_label(&self) -> Option<String> {
        match self.current_selected_content() {
            Some(SelectedContent::Track { track, .. }) => Some(track.title),
            Some(SelectedContent::Playlist(playlist)) => Some(playlist.title),
            Some(SelectedContent::User(user)) => Some(user.username),
            None => self.current_content_row().map(|row| row.columns[0].clone()),
        }
    }

    pub fn sync_route_from_library(&mut self) {
        if let Some(item) = self.library_items.get(self.selected_library) {
            self.set_route(item.route);
        }
    }

    pub fn sync_route_from_playlist(&mut self) {
        if self.playlists.is_empty() {
            return;
        }

        let playlist_index = self
            .selected_playlist
            .min(self.playlists.len().saturating_sub(1));
        self.selected_playlist = playlist_index;
        self.set_route(Route::Playlist(playlist_index));
    }

    pub fn set_route(&mut self, route: Route) {
        self.route = route;
        self.selected_content = 0;
        self.status = format!("Browsing {}.", self.route_title());

        if self.session.is_some() {
            self.request_route_load(false);
        } else {
            self.set_loading(format!("Loading mock data for {}...", self.route_title()));
        }
    }

    pub fn route_title(&self) -> String {
        match self.route {
            Route::Playlist(index) => self
                .playlists
                .get(index)
                .map(|playlist| playlist.title.clone())
                .unwrap_or_else(|| "Playlist".to_string()),
            _ => self.route.label().to_string(),
        }
    }

    pub fn select_current_content(&mut self) {
        match self.current_selected_content() {
            Some(SelectedContent::Track { track, context }) => {
                if self.session.is_some() {
                    if let Some((tracks, current_index)) = self.current_track_queue_selection() {
                        self.queue.tracks = tracks;
                        self.queue.current_index = Some(current_index);
                    } else {
                        self.queue.tracks = vec![track.clone()];
                        self.queue.current_index = Some(0);
                    }
                    self.start_track_playback(track, context);
                } else {
                    self.now_playing = NowPlaying {
                        track: Some(track.clone()),
                        title: track.title.clone(),
                        artist: track.artist.clone(),
                        context,
                        elapsed_label: "0:00".to_string(),
                        duration_label: track.duration_label(),
                        progress_ratio: 0.0,
                    };
                    self.status = format!(
                        "Selected {} for playback preview ({}).",
                        track.title,
                        track.access_label()
                    );
                }
            }
            Some(SelectedContent::Playlist(playlist)) => {
                self.status = format!("Inspected playlist {}.", playlist.title);
            }
            Some(SelectedContent::User(user)) => {
                self.status = format!("Inspected creator {}.", user.username);
            }
            None => {
                let Some(row) = self.current_content_row() else {
                    self.status = "Nothing selected in the content pane.".to_string();
                    return;
                };

                if self.route.is_track_view() {
                    self.now_playing = NowPlaying {
                        track: None,
                        title: row.columns[0].clone(),
                        artist: row.columns[1].clone(),
                        context: row.columns[2].clone(),
                        elapsed_label: "0:00".to_string(),
                        duration_label: row.columns[3].clone(),
                        progress_ratio: 0.0,
                    };
                    self.status = format!("Selected {} for mock playback preview.", row.columns[0]);
                } else {
                    self.status = format!("Inspected {}.", row.columns[0]);
                }
            }
        }
    }

    fn start_track_playback(&mut self, track: TrackSummary, context: String) {
        if !track.can_attempt_playback() {
            self.status = format!(
                "{} is blocked on SoundCloud and cannot be streamed.",
                track.title
            );
            return;
        }

        let Some(session) = self.session.clone() else {
            return;
        };

        self.player.status = PlaybackStatus::Buffering;
        self.player.position_seconds = 0.0;
        self.player.duration_seconds = track.duration_ms.map(|duration| duration as f64 / 1000.0);
        self.now_playing = NowPlaying {
            track: Some(track.clone()),
            title: track.title.clone(),
            artist: track.artist.clone(),
            context,
            elapsed_label: "0:00".to_string(),
            duration_label: track.duration_label(),
            progress_ratio: 0.0,
        };
        self.status = format!("Resolving SoundCloud stream for {}...", track.title);
        self.queue_command(AppCommand::PlayTrack { session, track });
    }

    fn current_track_queue_selection(&self) -> Option<(Vec<TrackSummary>, usize)> {
        match self.route {
            Route::Feed => {
                let mut queue = Vec::new();
                let mut selected_queue_index = None;

                for (row_index, item) in self.feed.items.iter().enumerate() {
                    if let FeedOrigin::Track(track) = &item.origin {
                        if row_index == self.selected_content {
                            selected_queue_index = Some(queue.len());
                        }
                        queue.push(track.clone());
                    }
                }

                selected_queue_index.map(|selected| (queue, selected))
            }
            Route::LikedSongs => Some((self.liked_tracks.items.clone(), self.selected_content)),
            Route::Playlist(index) => {
                let urn = self.playlists.get(index)?.urn.as_ref()?;
                let tracks = self.playlist_tracks.get(urn)?.items.clone();
                Some((tracks, self.selected_content))
            }
            Route::Search => Some((self.search_tracks.items.clone(), self.selected_content)),
            Route::RecentlyPlayed | Route::Albums | Route::Following => None,
        }
        .and_then(|(tracks, selected)| {
            if selected < tracks.len() {
                Some((tracks, selected))
            } else {
                None
            }
        })
    }

    pub fn toggle_playback(&mut self) {
        self.apply_playback_intent(PlaybackIntent::TogglePause);
    }

    pub fn apply_playback_intent(&mut self, intent: PlaybackIntent) {
        match intent {
            PlaybackIntent::Play => self.play_playback(),
            PlaybackIntent::Pause => self.pause_playback(),
            PlaybackIntent::TogglePause => {
                if self.player.status == PlaybackStatus::Stopped {
                    self.play_playback();
                } else if self.now_playing.track.is_none() {
                    self.status = "Select a track first.".to_string();
                } else {
                    self.queue_command(AppCommand::ControlPlayback(PlayerCommand::TogglePause));
                    self.status = "Toggling playback...".to_string();
                }
            }
            PlaybackIntent::Stop => self.stop_playback(),
            PlaybackIntent::Next => self.play_next_track(),
            PlaybackIntent::Previous => self.play_previous_track(),
            PlaybackIntent::SeekRelative { seconds } => self.seek_relative(seconds),
            PlaybackIntent::SeekAbsolute { seconds } => self.seek_absolute(seconds),
            PlaybackIntent::SetVolume { percent } => self.set_volume(percent),
            PlaybackIntent::SetShuffle(enabled) => self.set_shuffle(enabled),
            PlaybackIntent::SetRepeat(mode) => self.set_repeat_mode(mode),
        }
    }

    fn play_playback(&mut self) {
        let Some(track) = self.now_playing.track.clone() else {
            self.status = "Select a track first.".to_string();
            return;
        };

        match self.player.status {
            PlaybackStatus::Playing => {
                self.status = format!("{} is already playing.", track.title);
            }
            PlaybackStatus::Buffering => {
                self.status = format!("{} is still buffering.", track.title);
            }
            PlaybackStatus::Paused => {
                self.queue_command(AppCommand::ControlPlayback(PlayerCommand::Play));
                self.status = format!("Resuming {}...", track.title);
            }
            PlaybackStatus::Stopped => {
                self.start_track_playback(track, self.now_playing.context.clone());
            }
        }
    }

    fn pause_playback(&mut self) {
        let Some(title) = self
            .now_playing
            .track
            .as_ref()
            .map(|track| track.title.clone())
        else {
            self.status = "Nothing is playing.".to_string();
            return;
        };

        if self.player.status == PlaybackStatus::Paused {
            self.status = format!("{title} is already paused.");
            return;
        }

        if self.player.status == PlaybackStatus::Stopped {
            self.status = "Nothing is playing.".to_string();
            return;
        }

        self.queue_command(AppCommand::ControlPlayback(PlayerCommand::Pause));
        self.status = format!("Pausing {title}...");
    }

    fn stop_playback(&mut self) {
        if self.now_playing.track.is_none() {
            self.status = "Nothing is playing.".to_string();
            return;
        }

        self.queue_command(AppCommand::ControlPlayback(PlayerCommand::Stop));
        self.status = "Stopping playback...".to_string();
    }

    fn seek_relative(&mut self, seconds: f64) {
        if self.now_playing.track.is_none() {
            self.status = "Nothing is playing.".to_string();
            return;
        }

        self.queue_command(AppCommand::ControlPlayback(PlayerCommand::SeekRelative {
            seconds,
        }));
        self.status = if seconds.is_sign_negative() {
            format!("Seeking backward {:.0} seconds...", seconds.abs())
        } else {
            format!("Seeking forward {:.0} seconds...", seconds)
        };
    }

    fn seek_absolute(&mut self, seconds: f64) {
        if self.now_playing.track.is_none() {
            self.status = "Nothing is playing.".to_string();
            return;
        }

        let seconds = seconds.max(0.0);
        self.queue_command(AppCommand::ControlPlayback(PlayerCommand::SeekAbsolute {
            seconds,
        }));
        self.status = format!("Seeking to {}...", format_seconds_f64(seconds));
    }

    fn set_volume(&mut self, percent: f64) {
        let target = percent.clamp(0.0, 100.0);
        self.queue_command(AppCommand::ControlPlayback(PlayerCommand::SetVolume {
            percent: target,
        }));
        self.status = format!("Setting volume to {:.0}%...", target.round());
    }

    fn set_shuffle(&mut self, enabled: bool) {
        self.player.shuffle_enabled = enabled;
        self.status = if enabled {
            "Shuffle enabled.".to_string()
        } else {
            "Shuffle disabled.".to_string()
        };
    }

    fn set_repeat_mode(&mut self, repeat_mode: RepeatMode) {
        self.player.repeat_mode = repeat_mode;
        self.status = format!("Repeat mode set to {}.", repeat_mode.label());
    }

    fn play_next_track(&mut self) {
        let Some(next_index) = self.next_queue_index() else {
            self.status = "Reached the end of the queue.".to_string();
            return;
        };

        let Some(track) = self.queue.tracks.get(next_index).cloned() else {
            self.status = "Reached the end of the queue.".to_string();
            return;
        };

        self.queue.current_index = Some(next_index);
        self.start_track_playback(track, self.now_playing.context.clone());
    }

    fn next_queue_index(&self) -> Option<usize> {
        let current_index = self.queue.current_index?;
        let track_count = self.queue.tracks.len();

        if current_index + 1 < track_count {
            Some(current_index + 1)
        } else if self.player.repeat_mode == RepeatMode::Queue && track_count > 0 {
            Some(0)
        } else {
            None
        }
    }

    fn restart_current_track(&mut self) -> bool {
        let track = self
            .queue
            .current_index
            .and_then(|index| self.queue.tracks.get(index).cloned())
            .or_else(|| self.now_playing.track.clone());

        let Some(track) = track else {
            self.status = "Nothing queued for playback.".to_string();
            return false;
        };

        self.start_track_playback(track, self.now_playing.context.clone());
        true
    }

    fn play_previous_track(&mut self) {
        let Some(current_index) = self.queue.current_index else {
            self.status = "Nothing queued for playback.".to_string();
            return;
        };

        if self.player.position_seconds > 5.0 {
            self.queue_command(AppCommand::ControlPlayback(PlayerCommand::SeekAbsolute {
                seconds: 0.0,
            }));
            self.status = "Restarting the current track.".to_string();
            return;
        }

        let previous_index = if current_index > 0 {
            current_index - 1
        } else if self.player.repeat_mode == RepeatMode::Queue && !self.queue.tracks.is_empty() {
            self.queue.tracks.len().saturating_sub(1)
        } else {
            self.status = "Already at the start of the queue.".to_string();
            return;
        };

        let Some(track) = self.queue.tracks.get(previous_index).cloned() else {
            self.status = "Already at the start of the queue.".to_string();
            return;
        };

        self.queue.current_index = Some(previous_index);
        self.start_track_playback(track, self.now_playing.context.clone());
    }

    fn apply_player_event(&mut self, event: PlayerEvent) {
        match event {
            PlayerEvent::PlaybackStarted | PlayerEvent::PlaybackResumed => {
                self.player.status = PlaybackStatus::Playing;
                if let Some(track) = &self.now_playing.track {
                    self.status = format!("Playing {}.", track.title);
                }
            }
            PlayerEvent::PlaybackPaused => {
                self.player.status = PlaybackStatus::Paused;
                if let Some(track) = &self.now_playing.track {
                    self.status = format!("Paused {}.", track.title);
                }
            }
            PlayerEvent::PlaybackStopped => {
                self.player.status = PlaybackStatus::Stopped;
                self.player.position_seconds = 0.0;
                self.now_playing.elapsed_label = "0:00".to_string();
                self.now_playing.progress_ratio = 0.0;
                self.status = "Playback stopped.".to_string();
            }
            PlayerEvent::TrackEnded => {
                self.player.status = PlaybackStatus::Stopped;
                self.player.position_seconds = 0.0;
                self.now_playing.elapsed_label = "0:00".to_string();
                self.now_playing.progress_ratio = 0.0;
                if self.player.repeat_mode == RepeatMode::Track {
                    if !self.restart_current_track() {
                        self.status = "Reached the end of the queue.".to_string();
                    }
                } else if self.next_queue_index().is_some() {
                    self.play_next_track();
                } else {
                    self.status = "Reached the end of the queue.".to_string();
                }
            }
            PlayerEvent::PositionChanged { seconds } => {
                self.player.position_seconds = seconds;
                self.sync_now_playing_progress();
            }
            PlayerEvent::DurationChanged { seconds } => {
                self.player.duration_seconds = seconds;
                self.sync_now_playing_progress();
            }
            PlayerEvent::VolumeChanged { percent } => {
                self.player.volume_percent = percent.clamp(0.0, 100.0);
            }
            PlayerEvent::BackendError(error) => {
                self.player.status = PlaybackStatus::Stopped;
                self.status = format!("Playback backend error: {error}");
            }
        }
    }

    fn sync_now_playing_progress(&mut self) {
        self.now_playing.elapsed_label = format_seconds_f64(self.player.position_seconds);

        if let Some(duration_seconds) = self.player.duration_seconds {
            self.now_playing.duration_label = format_seconds_f64(duration_seconds);
            self.now_playing.progress_ratio = if duration_seconds <= 0.0 {
                0.0
            } else {
                (self.player.position_seconds / duration_seconds).clamp(0.0, 1.0)
            };
        }
    }

    pub fn maybe_queue_more_playlists(&mut self) -> bool {
        if self.session.is_none() || self.playlists_loading {
            return false;
        }

        let Some(next_href) = self.playlists_next_href.clone() else {
            return false;
        };
        let Some(session) = self.session.clone() else {
            return false;
        };

        self.playlists_loading = true;
        self.playlists_error = None;
        self.queue_command(AppCommand::LoadPlaylists {
            session,
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
                self.queue_command(AppCommand::LoadFeed {
                    session,
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
                self.queue_command(AppCommand::LoadLikedSongs {
                    session,
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
                self.queue_command(AppCommand::LoadAlbums {
                    session,
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
                self.queue_command(AppCommand::LoadFollowing {
                    session,
                    next_href: Some(next_href),
                    append: true,
                });
                self.status = "Loading more followed creators...".to_string();
                true
            }
            Route::Playlist(index) => {
                let Some(urn) = self
                    .playlists
                    .get(index)
                    .and_then(|playlist| playlist.urn.clone())
                else {
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
                self.queue_command(AppCommand::LoadPlaylistTracks {
                    session,
                    playlist_urn: urn,
                    next_href: Some(next_href),
                    append: true,
                });
                self.status = "Loading more playlist tracks...".to_string();
                true
            }
            Route::Search => {
                if self.search_tracks.loading {
                    return false;
                }
                let Some(next_href) = self.search_tracks.next_href.clone() else {
                    return false;
                };

                self.search_tracks.start_loading(true);
                self.queue_command(AppCommand::SearchTracksPage {
                    session,
                    query: self.search_query.clone(),
                    next_href,
                });
                self.status = format!("Loading more search results for '{}'...", self.search_query);
                true
            }
            Route::RecentlyPlayed => false,
        }
    }

    pub fn on_tick(&mut self) {
        self.tick_count = self.tick_count.saturating_add(1);

        if let Some(loading) = &mut self.loading {
            loading.ticks_remaining = loading.ticks_remaining.saturating_sub(1);
            if loading.ticks_remaining == 0 {
                self.loading = None;
            }
        }
    }

    pub fn on_resize(&mut self, width: u16, height: u16) {
        self.viewport = Viewport { width, height };
        self.status = match self.mode {
            AppMode::Auth => format!("Resized onboarding view to {}x{}.", width, height),
            AppMode::Main => format!(
                "Resized to {}x{} while focused on {}.",
                width,
                height,
                self.focus.label()
            ),
        };
    }

    pub fn set_loading(&mut self, message: impl Into<String>) {
        self.loading = Some(LoadingState {
            message: message.into(),
            ticks_remaining: 2,
        });
    }

    pub fn loading_label(&self) -> &str {
        self.loading
            .as_ref()
            .map(|loading| loading.message.as_str())
            .unwrap_or("Ready")
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        if is_global_quit_key(key) {
            self.should_quit = true;
            return;
        }

        match self.mode {
            AppMode::Auth => {
                if let Some(intent) = self.auth.handle_key(key) {
                    self.handle_auth_intent(intent);
                }
            }
            AppMode::Main => {
                if self.focus == Focus::Search && self.handle_search_key(key) {
                    return;
                }

                if self.handle_playback_key(key) {
                    return;
                }

                if let Some(action) = map_main_key_event(key) {
                    self.apply(action);
                }
            }
        }
    }

    fn handle_paste_event(&mut self, text: &str) {
        match self.mode {
            AppMode::Auth => {
                self.auth.paste_text(text);
                self.status = "Pasted clipboard contents into the active field.".to_string();
            }
            AppMode::Main if self.focus == Focus::Search => {
                let sanitized = text.replace(['\r', '\n'], " ");
                self.insert_search_text(sanitized.trim());
                self.status = format!("Updated search query to '{}'.", self.search_query);
            }
            AppMode::Main => {}
        }
    }

    fn handle_search_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Enter => {
                self.submit_search();
                true
            }
            KeyCode::Left => {
                self.search_cursor = self.search_cursor.saturating_sub(1);
                true
            }
            KeyCode::Right => {
                self.search_cursor =
                    (self.search_cursor + 1).min(self.search_query.chars().count());
                true
            }
            KeyCode::Home => {
                self.search_cursor = 0;
                true
            }
            KeyCode::End => {
                self.search_cursor = self.search_query.chars().count();
                true
            }
            KeyCode::Backspace => {
                self.backspace_search();
                true
            }
            KeyCode::Delete => {
                self.delete_search();
                true
            }
            KeyCode::Char(ch) if key.modifiers == KeyModifiers::NONE => {
                self.insert_search_char(ch);
                true
            }
            KeyCode::Char('u') if key.modifiers == KeyModifiers::CONTROL => {
                self.search_query.clear();
                self.search_cursor = 0;
                self.status = "Cleared the search query.".to_string();
                true
            }
            _ => false,
        }
    }

    fn handle_playback_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char(' ') => {
                self.apply_playback_intent(PlaybackIntent::TogglePause);
                true
            }
            KeyCode::Char('s') => {
                self.apply_playback_intent(PlaybackIntent::Stop);
                true
            }
            KeyCode::Char('n') => {
                self.apply_playback_intent(PlaybackIntent::Next);
                true
            }
            KeyCode::Char('p') => {
                self.apply_playback_intent(PlaybackIntent::Previous);
                true
            }
            KeyCode::Left => {
                self.apply_playback_intent(PlaybackIntent::SeekRelative { seconds: -5.0 });
                true
            }
            KeyCode::Right => {
                self.apply_playback_intent(PlaybackIntent::SeekRelative { seconds: 5.0 });
                true
            }
            KeyCode::Char('-') => {
                self.apply_playback_intent(PlaybackIntent::SetVolume {
                    percent: self.player.volume_percent - 5.0,
                });
                true
            }
            KeyCode::Char('=') | KeyCode::Char('+') => {
                self.apply_playback_intent(PlaybackIntent::SetVolume {
                    percent: self.player.volume_percent + 5.0,
                });
                true
            }
            _ => false,
        }
    }

    fn handle_auth_intent(&mut self, intent: AuthIntent) {
        match intent {
            AuthIntent::OpenAppsPage => {
                self.status = "Opening SoundCloud app registration in your browser...".to_string();
                self.queue_command(AppCommand::OpenUrl(
                    "https://soundcloud.com/you/apps".to_string(),
                ));
            }
            AuthIntent::SaveAndContinue => {
                let credentials = self.auth.credentials();
                match credentials.validate() {
                    Ok(()) => match crate::soundcloud::auth::prepare_authorization(credentials) {
                        Ok(request) => {
                            self.auth.clear_error();
                            self.auth.set_info(
                                "Saving your SoundCloud app credentials locally before opening the browser.",
                            );
                            self.set_loading("Saving SoundCloud credentials locally...");
                            self.status =
                                "Saving your SoundCloud credentials locally...".to_string();
                            self.queue_command(AppCommand::SaveCredentials(request));
                        }
                        Err(error) => {
                            self.auth.set_error(error.to_string());
                            self.status = error.to_string();
                        }
                    },
                    Err(error) => {
                        self.auth.set_error(error.to_string());
                        self.status = error.to_string();
                    }
                }
            }
            AuthIntent::OpenBrowser => {
                if let Some(url) = &self.auth.auth_url {
                    self.status =
                        "Opening the SoundCloud authorize page in your browser...".to_string();
                    self.queue_command(AppCommand::OpenUrl(url.clone()));
                }
            }
            AuthIntent::ShowManualCallback => {
                self.auth.show_manual_callback(
                    "Paste the full callback URL from your browser after approving SoundCloud access.",
                );
                self.status = "Waiting for manual callback URL entry.".to_string();
            }
            AuthIntent::BackToCredentials => {
                self.auth.back_to_credentials();
                self.loading = None;
                self.status = "Edit your credentials and try again.".to_string();
            }
            AuthIntent::SubmitManualCallback => {
                if let Some(request) = self.auth.pending_authorization.clone() {
                    self.auth.clear_error();
                    self.set_loading("Submitting the pasted callback URL...");
                    self.queue_command(AppCommand::ExchangeAuthorizationCode {
                        request,
                        callback_input: self.auth.callback_input.value.clone(),
                    });
                }
            }
            AuthIntent::BackToBrowser => {
                self.auth.step = crate::app::AuthStep::WaitingForBrowser;
                self.auth.focus = crate::app::AuthFocus::OpenBrowser;
                self.status = "Waiting for the browser callback again.".to_string();
            }
        }
    }

    fn complete_auth(&mut self, session: AuthorizedSession) {
        self.mode = AppMode::Main;
        self.loading = None;
        self.session = Some(session.clone());
        self.set_auth_session(&session);
        self.reset_live_data();
        self.request_playlists_load(false);
        self.request_route_load(false);
    }

    fn queue_command(&mut self, command: AppCommand) {
        self.pending_commands.push(command);
    }

    fn request_playlists_load(&mut self, append: bool) {
        let Some(session) = self.session.clone() else {
            return;
        };

        if self.playlists_loading || (!append && self.playlists_loaded) {
            return;
        }

        self.playlists_loading = true;
        self.playlists_error = None;
        self.queue_command(AppCommand::LoadPlaylists {
            session,
            next_href: if append {
                self.playlists_next_href.clone()
            } else {
                None
            },
            append,
        });
    }

    fn request_route_load(&mut self, append: bool) {
        let Some(session) = self.session.clone() else {
            return;
        };

        match self.route {
            Route::Feed => {
                if self.feed.loading || (!append && self.feed.loaded) {
                    return;
                }
                self.feed.start_loading(append);
                self.queue_command(AppCommand::LoadFeed {
                    session,
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
                self.queue_command(AppCommand::LoadLikedSongs {
                    session,
                    next_href: if append {
                        self.liked_tracks.next_href.clone()
                    } else {
                        None
                    },
                    append,
                });
            }
            Route::RecentlyPlayed => {}
            Route::Albums => {
                if self.albums.loading || (!append && self.albums.loaded) {
                    return;
                }
                self.albums.start_loading(append);
                self.queue_command(AppCommand::LoadAlbums {
                    session,
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
                self.queue_command(AppCommand::LoadFollowing {
                    session,
                    next_href: if append {
                        self.following.next_href.clone()
                    } else {
                        None
                    },
                    append,
                });
            }
            Route::Playlist(index) => {
                let Some(urn) = self
                    .playlists
                    .get(index)
                    .and_then(|playlist| playlist.urn.clone())
                else {
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
                self.queue_command(AppCommand::LoadPlaylistTracks {
                    session,
                    playlist_urn: urn,
                    next_href,
                    append,
                });
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
                self.search_tracks.start_loading(false);
                self.search_playlists.start_loading(false);
                self.search_users.start_loading(false);
                self.queue_command(AppCommand::SearchAll {
                    session,
                    query: self.search_query.clone(),
                });
            }
        }
    }

    fn reset_live_data(&mut self) {
        self.playlists.clear();
        self.playlists_loading = false;
        self.playlists_loaded = false;
        self.playlists_error = None;
        self.playlists_next_href = None;
        self.feed = CollectionState::default();
        self.liked_tracks = CollectionState::default();
        self.albums = CollectionState::default();
        self.following = CollectionState::default();
        self.playlist_tracks.clear();
        self.search_tracks = CollectionState::default();
        self.search_playlists = CollectionState::default();
        self.search_users = CollectionState::default();
        self.selected_playlist = 0;
        self.selected_content = 0;
        self.queue = QueueState::default();
        self.player = PlayerState {
            status: PlaybackStatus::Stopped,
            volume_percent: 50.0,
            position_seconds: 0.0,
            duration_seconds: None,
            shuffle_enabled: false,
            repeat_mode: RepeatMode::Off,
        };
        self.now_playing.track = None;
        self.now_playing.progress_ratio = 0.0;
        self.now_playing.elapsed_label = "0:00".to_string();
        self.now_playing.duration_label = "0:00".to_string();
    }

    fn apply_playlists_page(&mut self, page: Page<SoundcloudPlaylist>, append: bool) {
        self.playlists_loading = false;
        self.playlists_loaded = true;
        self.playlists_error = None;
        self.playlists_next_href = page.next_href.clone();

        let mapped = page
            .items
            .into_iter()
            .map(|playlist| SidebarPlaylist {
                urn: Some(playlist.urn),
                title: playlist.title,
                description: playlist.description,
                creator: Some(playlist.creator),
                track_count: Some(playlist.track_count),
                tracks: Vec::new(),
            })
            .collect::<Vec<_>>();

        if append {
            self.playlists.extend(mapped);
        } else {
            self.playlists = mapped;
        }

        if self.playlists.is_empty() {
            self.selected_playlist = 0;
            if matches!(self.route, Route::Playlist(_)) {
                self.route = Route::Feed;
            }
        } else {
            self.selected_playlist = self.selected_playlist.min(self.playlists.len() - 1);
        }

        self.status = format!("Loaded {} playlists.", self.playlists.len());
    }

    fn apply_search_results(&mut self, results: SearchResults) {
        self.search_tracks.apply_page(results.tracks, false);
        self.search_playlists.apply_page(results.playlists, false);
        self.search_users.apply_page(results.users, false);
    }

    fn current_selected_content(&self) -> Option<SelectedContent> {
        let index = self.selected_content;

        if self.session.is_none() {
            return None;
        }

        match self.route {
            Route::Feed => self
                .feed
                .items
                .get(index)
                .and_then(|item| match &item.origin {
                    FeedOrigin::Track(track) => Some(SelectedContent::Track {
                        track: track.clone(),
                        context: pretty_activity_type(&item.activity_type),
                    }),
                    FeedOrigin::Playlist(playlist) => {
                        Some(SelectedContent::Playlist(playlist.clone()))
                    }
                }),
            Route::LikedSongs => {
                self.liked_tracks
                    .items
                    .get(index)
                    .cloned()
                    .map(|track| SelectedContent::Track {
                        track,
                        context: "Liked Songs".to_string(),
                    })
            }
            Route::RecentlyPlayed => None,
            Route::Albums => self
                .albums
                .items
                .get(index)
                .cloned()
                .map(SelectedContent::Playlist),
            Route::Following => self
                .following
                .items
                .get(index)
                .cloned()
                .map(SelectedContent::User),
            Route::Playlist(route_index) => self
                .playlists
                .get(route_index)
                .and_then(|playlist| playlist.urn.as_ref())
                .and_then(|urn| self.playlist_tracks.get(urn))
                .and_then(|state| state.items.get(index))
                .cloned()
                .map(|track| SelectedContent::Track {
                    context: self.route_title(),
                    track,
                }),
            Route::Search => {
                self.search_tracks
                    .items
                    .get(index)
                    .cloned()
                    .map(|track| SelectedContent::Track {
                        track,
                        context: format!("Search: {}", self.search_query),
                    })
            }
        }
    }

    fn submit_search(&mut self) {
        let query = self.search_query.trim().to_string();
        if query.is_empty() {
            self.status = "Enter a search query first.".to_string();
            return;
        }

        self.search_query = query;
        self.search_cursor = self.search_query.chars().count();
        self.route = Route::Search;
        self.selected_content = 0;
        self.status = format!("Searching SoundCloud for '{}'...", self.search_query);
        self.request_route_load(false);
    }

    fn insert_search_char(&mut self, ch: char) {
        let mut chars = self.search_query.chars().collect::<Vec<_>>();
        chars.insert(self.search_cursor, ch);
        self.search_query = chars.into_iter().collect();
        self.search_cursor += 1;
    }

    fn insert_search_text(&mut self, text: &str) {
        for ch in text.chars() {
            self.insert_search_char(ch);
        }
    }

    fn backspace_search(&mut self) {
        if self.search_cursor == 0 {
            return;
        }

        let mut chars = self.search_query.chars().collect::<Vec<_>>();
        chars.remove(self.search_cursor - 1);
        self.search_query = chars.into_iter().collect();
        self.search_cursor -= 1;
    }

    fn delete_search(&mut self) {
        let mut chars = self.search_query.chars().collect::<Vec<_>>();
        if self.search_cursor >= chars.len() {
            return;
        }

        chars.remove(self.search_cursor);
        self.search_query = chars.into_iter().collect();
    }

    fn mock_content(&self) -> ContentView {
        match self.route {
            Route::Feed => ContentView {
                title: "Feed".to_string(),
                subtitle: "Fresh picks from the accounts you follow.".to_string(),
                columns: ["Title", "Artist", "Source", "Length"],
                rows: self.feed_rows.clone(),
                state_label: "Mock data".to_string(),
                empty_message: "No mock feed rows available.".to_string(),
            },
            Route::LikedSongs => ContentView {
                title: "Liked Songs".to_string(),
                subtitle: "Mock favorites pinned for the shell.".to_string(),
                columns: ["Title", "Artist", "Collection", "Length"],
                rows: self.liked_rows.clone(),
                state_label: "Mock data".to_string(),
                empty_message: "No mock liked songs available.".to_string(),
            },
            Route::RecentlyPlayed => ContentView {
                title: "Recently Played".to_string(),
                subtitle: "Local history will become real in a later phase.".to_string(),
                columns: ["Title", "Artist", "Last Context", "Length"],
                rows: self.recent_rows.clone(),
                state_label: "Mock data".to_string(),
                empty_message: "No mock history available.".to_string(),
            },
            Route::Albums => ContentView {
                title: "Albums".to_string(),
                subtitle: "Album-like sets surfaced as a dedicated mock view.".to_string(),
                columns: ["Album", "Creator", "Tracks", "Year"],
                rows: self.album_rows.clone(),
                state_label: "Mock data".to_string(),
                empty_message: "No mock albums available.".to_string(),
            },
            Route::Following => ContentView {
                title: "Following".to_string(),
                subtitle: "Mock creator directory with a few followed accounts.".to_string(),
                columns: ["Creator", "Followers", "Spotlight", "Status"],
                rows: self.following_rows.clone(),
                state_label: "Mock data".to_string(),
                empty_message: "No mock following rows available.".to_string(),
            },
            Route::Playlist(index) => {
                let playlist = &self.playlists[index];
                ContentView {
                    title: playlist.title.clone(),
                    subtitle: playlist.description.clone(),
                    columns: ["Title", "Artist", "Playlist", "Length"],
                    rows: playlist.tracks.clone(),
                    state_label: "Mock data".to_string(),
                    empty_message: "No mock playlist tracks available.".to_string(),
                }
            }
            Route::Search => ContentView {
                title: format!("Search: {}", self.search_query),
                subtitle: "Mock search results until the real API layer lands.".to_string(),
                columns: ["Title", "Artist", "Collection", "Length"],
                rows: self.search_rows.clone(),
                state_label: "Mock data".to_string(),
                empty_message: "No mock search results available.".to_string(),
            },
        }
    }
}

fn mock_playlists() -> Vec<SidebarPlaylist> {
    vec![
        SidebarPlaylist {
            urn: None,
            title: "Sunset Drive".to_string(),
            description: "Warm house and road-trip cuts".to_string(),
            creator: None,
            track_count: None,
            tracks: mock_track_rows(&[
                ("Golden Hour", "Tycho", "Sunset Drive", "4:12"),
                ("La Mar", "Brijean", "Sunset Drive", "3:38"),
                ("Kites", "Bonobo", "Sunset Drive", "5:14"),
                ("Silk Route", "Ross From Friends", "Sunset Drive", "4:41"),
            ]),
        },
        SidebarPlaylist {
            urn: None,
            title: "Low Light".to_string(),
            description: "Late-night electronics and downtempo".to_string(),
            creator: None,
            track_count: None,
            tracks: mock_track_rows(&[
                ("Night Bloom", "Tourist", "Low Light", "3:56"),
                ("Shoreline", "Ford.", "Low Light", "4:05"),
                ("Blink", "Four Tet", "Low Light", "4:24"),
                ("Shiver", "Catching Flies", "Low Light", "3:49"),
            ]),
        },
        SidebarPlaylist {
            urn: None,
            title: "Warehouse Mornings".to_string(),
            description: "Minimal grooves for long focus blocks".to_string(),
            creator: None,
            track_count: None,
            tracks: mock_track_rows(&[
                ("Tracer", "Djoko", "Warehouse Mornings", "6:18"),
                ("Pebble", "Bicep", "Warehouse Mornings", "5:02"),
                ("Sunline", "Logic1000", "Warehouse Mornings", "4:47"),
                ("Lifted", "Mall Grab", "Warehouse Mornings", "5:23"),
            ]),
        },
        SidebarPlaylist {
            urn: None,
            title: "Cloud Sketches".to_string(),
            description: "Ambient drafts and instrumental loops".to_string(),
            creator: None,
            track_count: None,
            tracks: mock_track_rows(&[
                ("Paper Sky", "Helios", "Cloud Sketches", "3:18"),
                ("Still Water", "Hania Rani", "Cloud Sketches", "4:32"),
                ("Moss", "Kaitlyn Aurelia Smith", "Cloud Sketches", "5:07"),
                ("Resin", "Rival Consoles", "Cloud Sketches", "4:28"),
            ]),
        },
    ]
}

fn mock_track_rows(items: &[(&str, &str, &str, &str)]) -> Vec<ContentRow> {
    items
        .iter()
        .map(|(title, artist, collection, length)| ContentRow {
            columns: [
                (*title).to_string(),
                (*artist).to_string(),
                (*collection).to_string(),
                (*length).to_string(),
            ],
        })
        .collect()
}

fn format_seconds_f64(seconds: f64) -> String {
    let seconds = seconds.max(0.0).round() as u64;
    let minutes = seconds / 60;
    let remainder = seconds % 60;
    format!("{minutes}:{remainder:02}")
}

fn pretty_activity_type(activity_type: &str) -> String {
    activity_type
        .replace('_', " ")
        .split_whitespace()
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn track_row_with_access(track: &TrackSummary) -> ContentRow {
    ContentRow {
        columns: [
            track.title.clone(),
            track.artist.clone(),
            track.access_label().to_string(),
            track.duration_label(),
        ],
    }
}

fn sidebar_playlist_subtitle(playlist: &SidebarPlaylist) -> String {
    if !playlist.description.trim().is_empty() {
        playlist.description.clone()
    } else {
        match (&playlist.creator, playlist.track_count) {
            (Some(creator), Some(track_count)) => {
                format!("By {} - {} tracks", creator, track_count)
            }
            (Some(creator), None) => format!("By {}", creator),
            (None, Some(track_count)) => format!("{} tracks", track_count),
            (None, None) => "Playlist details".to_string(),
        }
    }
}
