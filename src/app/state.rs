use std::collections::HashMap;

use chrono::Utc;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{
    app::{
        Action, AppCommand, AppEvent, AppMode, AuthIntent, AuthState, Focus, PlaybackIntent,
        RepeatMode, Route, SettingsMenuState, reducer,
    },
    config::{
        credentials::Credentials,
        history::{RecentlyPlayedEntry, RecentlyPlayedStore},
        settings::{KeyAction, Settings, StartupBehavior},
        tokens::TokenStore,
    },
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
    ui::theme::Theme,
};

#[derive(Debug, Clone)]
pub struct AppState {
    pub mode: AppMode,
    pub route: Route,
    pub focus: Focus,
    pub should_quit: bool,
    pub show_help: bool,
    pub settings_menu: Option<SettingsMenuState>,
    pub show_welcome: bool,
    pub error_modal: Option<ErrorModal>,
    pub add_to_playlist_modal: Option<AddToPlaylistModal>,
    pub toast: Option<Toast>,
    pub help_scroll: usize,
    pub auth: AuthState,
    pub session: Option<AuthorizedSession>,
    pub auth_summary: String,
    pub status: String,
    pub tick_count: u64,
    pub viewport: Viewport,
    pub loading: Option<LoadingState>,
    pub search_query: String,
    pub search_cursor: usize,
    pub search_return_focus: Focus,
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
    pub layout: LayoutState,
    pub now_playing: NowPlaying,
    pub cover_art: CoverArt,
    pub player: PlayerState,
    pub queue: QueueState,
    settings: Settings,
    help_requires_acknowledgement: bool,
    content_return_focus: Focus,
    pending_commands: Vec<AppCommand>,
    recent_history: RecentlyPlayedStore,
    active_playlist_urn: Option<String>,
    known_playlists: HashMap<String, SoundcloudPlaylist>,
    feed: CollectionState<FeedItem>,
    liked_tracks: CollectionState<TrackSummary>,
    albums: CollectionState<SoundcloudPlaylist>,
    following: CollectionState<UserSummary>,
    playlist_tracks: HashMap<String, CollectionState<TrackSummary>>,
    search_tracks: CollectionState<TrackSummary>,
    search_playlists: CollectionState<SoundcloudPlaylist>,
    search_users: CollectionState<UserSummary>,
    search_view: SearchView,
    active_user_profile: Option<UserSummary>,
    user_profile_tracks: CollectionState<TrackSummary>,
    user_profile_playlists: CollectionState<SoundcloudPlaylist>,
    user_profile_view: UserProfileView,
    search_cache: HashMap<String, SearchCache>,
    playback_plan: PlaybackPlanState,
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

#[derive(Debug, Clone, Copy)]
pub struct LayoutState {
    pub sidebar_width_percent: u16,
    pub library_height: u16,
    pub playbar_height: u16,
}

impl Default for LayoutState {
    fn default() -> Self {
        Self {
            sidebar_width_percent: 20,
            library_height: 7,
            playbar_height: 6,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HelpRow {
    pub description: String,
    pub event: String,
    pub context: String,
}

#[derive(Debug, Clone)]
pub struct ErrorModal {
    pub title: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct AddToPlaylistModal {
    pub track: TrackSummary,
    pub selected_playlist: usize,
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub expires_at_tick: u64,
}

#[derive(Debug, Clone)]
pub struct NowPlaying {
    pub track: Option<TrackSummary>,
    pub title: String,
    pub artist: String,
    pub context: String,
    pub artwork_url: Option<String>,
    pub elapsed_label: String,
    pub duration_label: String,
    pub progress_ratio: f64,
}

#[derive(Debug, Clone, Default)]
pub struct CoverArt {
    pub url: Option<String>,
    pub bytes: Option<Vec<u8>>,
    pub loading: bool,
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
    pub overlay_visible: bool,
    pub selected: usize,
}

#[derive(Debug, Clone, Default)]
struct PlaybackPlanState {
    items: Vec<PlaybackPlanItem>,
    current_index: Option<usize>,
}

#[derive(Debug, Clone)]
struct PlaybackPlanItem {
    track: TrackSummary,
    context: String,
    kind: PlaybackPlanItemKind,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum PlaybackPlanItemKind {
    Source,
    Queue,
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
    pub help_message: Option<String>,
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

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
enum SearchView {
    #[default]
    Tracks,
    Playlists,
    Users,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
enum UserProfileView {
    #[default]
    Tracks,
    Playlists,
}

#[derive(Debug, Clone)]
struct SearchCache {
    tracks: CollectionState<TrackSummary>,
    playlists: CollectionState<SoundcloudPlaylist>,
    users: CollectionState<UserSummary>,
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
        } else if self.error.is_some() {
            "Error".to_string()
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

impl SearchView {
    fn label(self) -> &'static str {
        match self {
            Self::Tracks => "Tracks",
            Self::Playlists => "Playlists",
            Self::Users => "Users",
        }
    }
}

impl UserProfileView {
    fn label(self) -> &'static str {
        match self {
            Self::Tracks => "Tracks",
            Self::Playlists => "Playlists",
        }
    }
}

impl SearchCache {
    fn from_state(app: &AppState) -> Self {
        Self {
            tracks: app.search_tracks.clone(),
            playlists: app.search_playlists.clone(),
            users: app.search_users.clone(),
        }
    }
}

impl AppState {
    pub fn new() -> Self {
        Self::new_with_persistence(Settings::default(), RecentlyPlayedStore::default())
    }

    pub fn new_with_persistence(
        mut settings: Settings,
        recent_history: RecentlyPlayedStore,
    ) -> Self {
        settings.normalize();
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
            show_help: false,
            settings_menu: None,
            show_welcome: true,
            error_modal: None,
            add_to_playlist_modal: None,
            toast: None,
            help_scroll: 0,
            auth: AuthState::new(Credentials::default()),
            session: None,
            auth_summary: "Unauthenticated".to_string(),
            status: "Tab cycles panes, / opens search, z queues selected, Q opens queue, w/l use selected track, W/L use now playing, ? opens help, q quits."
                .to_string(),
            tick_count: 0,
            viewport: Viewport {
                width: 0,
                height: 0,
            },
            loading: Some(LoadingState {
                message: "Preparing mock library data...".to_string(),
                ticks_remaining: 2,
            }),
            search_query: String::new(),
            search_cursor: 0,
            search_return_focus: Focus::Library,
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
            layout: LayoutState::default(),
            now_playing: NowPlaying {
                track: None,
                title: "Nothing playing".to_string(),
                artist: "Select a track and press Enter".to_string(),
                context: "Idle".to_string(),
                artwork_url: None,
                elapsed_label: "0:00".to_string(),
                duration_label: "0:00".to_string(),
                progress_ratio: 0.0,
            },
            cover_art: CoverArt::default(),
            player: PlayerState {
                status: PlaybackStatus::Stopped,
                volume_percent: 50.0,
                position_seconds: 0.0,
                duration_seconds: None,
                shuffle_enabled: false,
                repeat_mode: RepeatMode::Off,
            },
            queue: QueueState::default(),
            settings,
            help_requires_acknowledgement: false,
            content_return_focus: Focus::Library,
            pending_commands: Vec::new(),
            recent_history,
            active_playlist_urn: None,
            known_playlists: HashMap::new(),
            feed: CollectionState::default(),
            liked_tracks: CollectionState::default(),
            albums: CollectionState::default(),
            following: CollectionState::default(),
            playlist_tracks: HashMap::new(),
            search_tracks: CollectionState::default(),
            search_playlists: CollectionState::default(),
            search_users: CollectionState::default(),
            search_view: SearchView::Tracks,
            active_user_profile: None,
            user_profile_tracks: CollectionState::default(),
            user_profile_playlists: CollectionState::default(),
            user_profile_view: UserProfileView::Tracks,
            search_cache: HashMap::new(),
            playback_plan: PlaybackPlanState::default(),
            playlists_loading: false,
            playlists_loaded: false,
            playlists_error: None,
            playlists_next_href: None,
        }
    }

    pub fn new_onboarding(credentials: Credentials) -> Self {
        Self::new_onboarding_with_persistence(
            credentials,
            Settings::default(),
            RecentlyPlayedStore::default(),
        )
    }

    pub fn new_onboarding_with_persistence(
        credentials: Credentials,
        settings: Settings,
        recent_history: RecentlyPlayedStore,
    ) -> Self {
        let mut app = Self::new_with_persistence(settings, recent_history);
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
                help_message: Some(
                    "Enter plays tracks and opens playlists when the feed row points to one."
                        .to_string(),
                ),
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
                help_message: Some("Press Enter to play from your liked queue.".to_string()),
            },
            Route::RecentlyPlayed => ContentView {
                title: "Recently Played".to_string(),
                subtitle: "Local playback history stored on this machine.".to_string(),
                columns: ["Title", "Artist", "Last Context", "Played"],
                rows: self
                    .recent_history
                    .entries
                    .iter()
                    .map(history_row)
                    .collect(),
                state_label: self.recent_history_state_label(),
                empty_message: "Play a track to start building local history.".to_string(),
                help_message: Some(
                    "Recently Played persists across restarts and seeds a playable queue."
                        .to_string(),
                ),
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
                help_message: Some("Press Enter to open an album as playlist detail.".to_string()),
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
                help_message: Some(
                    "Press Enter to open a followed creator profile in the TUI.".to_string(),
                ),
            },
            Route::Playlist => {
                let playlist = self.active_playlist();
                let tracks = playlist
                    .map(|playlist| playlist.urn.as_str())
                    .and_then(|urn| self.playlist_tracks.get(urn));

                ContentView {
                    title: playlist
                        .map(|playlist| playlist.title.clone())
                        .unwrap_or_else(|| "Playlist".to_string()),
                    subtitle: playlist
                        .map(playlist_summary_subtitle)
                        .unwrap_or_else(|| "Playlist details are loading.".to_string()),
                    columns: ["Title", "Artist", "Access", "Length"],
                    rows: tracks
                        .map(|state| state.items.iter().map(track_row_with_access).collect())
                        .unwrap_or_default(),
                    state_label: tracks
                        .map(CollectionState::state_label)
                        .unwrap_or_else(|| "Waiting".to_string()),
                    empty_message: "No tracks are available for this playlist.".to_string(),
                    help_message: Some(
                        "Enter plays from this playlist queue. Esc returns to the prior pane."
                            .to_string(),
                    ),
                }
            }
            Route::UserProfile => {
                let Some(user) = self.active_user_profile.as_ref() else {
                    return ContentView {
                        title: "Profile".to_string(),
                        subtitle: "Select a creator from Following or Search to open a profile."
                            .to_string(),
                        columns: ["Title", "Artist", "Access", "Length"],
                        rows: Vec::new(),
                        state_label: "Waiting".to_string(),
                        empty_message: "No creator profile is currently open.".to_string(),
                        help_message: Some(
                            "Press 1 for tracks and 2 for playlists once a profile is open."
                                .to_string(),
                        ),
                    };
                };

                match self.user_profile_view {
                    UserProfileView::Tracks => ContentView {
                        title: format!("{} - Tracks", user.username),
                        subtitle: self.user_profile_subtitle(user),
                        columns: ["Title", "Artist", "Access", "Length"],
                        rows: self
                            .user_profile_tracks
                            .items
                            .iter()
                            .map(track_row_with_access)
                            .collect(),
                        state_label: self.user_profile_tracks.state_label(),
                        empty_message: format!(
                            "No public tracks are available for {}.",
                            user.username
                        ),
                        help_message: Some(
                            "1 tracks | 2 playlists | Enter plays the selected track.".to_string(),
                        ),
                    },
                    UserProfileView::Playlists => ContentView {
                        title: format!("{} - Playlists", user.username),
                        subtitle: self.user_profile_subtitle(user),
                        columns: ["Playlist", "Creator", "Tracks", "Year"],
                        rows: self
                            .user_profile_playlists
                            .items
                            .iter()
                            .map(playlist_row)
                            .collect(),
                        state_label: self.user_profile_playlists.state_label(),
                        empty_message: format!(
                            "No public playlists are available for {}.",
                            user.username
                        ),
                        help_message: Some(
                            "1 tracks | 2 playlists | Enter opens the selected playlist."
                                .to_string(),
                        ),
                    },
                }
            }
            Route::Search => match self.search_view {
                SearchView::Tracks => ContentView {
                    title: self.search_title(),
                    subtitle: self.search_subtitle(),
                    columns: ["Title", "Artist", "Access", "Length"],
                    rows: self
                        .search_tracks
                        .items
                        .iter()
                        .map(track_row_with_access)
                        .collect(),
                    state_label: self.search_tracks.state_label(),
                    empty_message: "No matching tracks were found.".to_string(),
                    help_message: Some(self.search_help_message()),
                },
                SearchView::Playlists => ContentView {
                    title: self.search_title(),
                    subtitle: self.search_subtitle(),
                    columns: ["Playlist", "Creator", "Tracks", "Year"],
                    rows: self
                        .search_playlists
                        .items
                        .iter()
                        .map(playlist_row)
                        .collect(),
                    state_label: self.search_playlists.state_label(),
                    empty_message: "No matching playlists were found.".to_string(),
                    help_message: Some(self.search_help_message()),
                },
                SearchView::Users => ContentView {
                    title: self.search_title(),
                    subtitle: self.search_subtitle(),
                    columns: ["Creator", "Followers", "Catalog", "Profile"],
                    rows: self.search_users.items.iter().map(user_row).collect(),
                    state_label: self.search_users.state_label(),
                    empty_message: "No matching creators were found.".to_string(),
                    help_message: Some(self.search_help_message()),
                },
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

    pub fn header_help_label(&self) -> String {
        if self.show_help {
            format!(
                "Esc closes help | {}/{} scroll | {} toggles",
                self.settings.keybinding(KeyAction::NextPage),
                self.settings.keybinding(KeyAction::PreviousPage),
                self.settings.keybinding(KeyAction::Help)
            )
        } else {
            format!(
                "{} help | {} search | {} queue | {} overlay | w/l selected | W/L current | Tab panes | j/k move | Enter select | q quit",
                self.settings.keybinding(KeyAction::Help),
                self.settings.keybinding(KeyAction::Search),
                self.settings.keybinding(KeyAction::AddToQueue),
                self.settings.keybinding(KeyAction::ShowQueue)
            )
        }
    }

    pub fn help_rows(&self) -> Vec<HelpRow> {
        vec![
            help_row("Move focus to next pane", "Tab", "Navigation"),
            help_row("Move focus to previous pane", "Shift+Tab", "Navigation"),
            help_row("Move selection down", "j | Down", "Navigation"),
            help_row("Move selection up", "k | Up", "Navigation"),
            help_row("Enter active pane", "Enter", "Navigation"),
            help_row(
                "Open help menu",
                format!("{} | F1", self.settings.keybinding(KeyAction::Help)),
                "General",
            ),
            help_row(
                "Enter input for search",
                self.settings.keybinding(KeyAction::Search),
                "General",
            ),
            help_row(
                "Pause/Resume playback",
                self.settings.keybinding(KeyAction::TogglePlayback),
                "General",
            ),
            help_row(
                "Open queue overlay",
                self.settings.keybinding(KeyAction::ShowQueue),
                "General",
            ),
            help_row("Add selected track to playlist", "w", "Content"),
            help_row("Add selected track to Liked Songs", "l", "Content"),
            help_row(
                "Add selected track to queue",
                self.settings.keybinding(KeyAction::AddToQueue),
                "Content",
            ),
            help_row("Add current now playing track to playlist", "W", "General"),
            help_row(
                "Add current now playing track to Liked Songs",
                "L",
                "General",
            ),
            help_row(
                "Skip to next track",
                self.settings.keybinding(KeyAction::NextTrack),
                "General",
            ),
            help_row(
                "Skip to previous track",
                self.settings.keybinding(KeyAction::PreviousTrack),
                "General",
            ),
            help_row(
                "Seek backwards",
                self.settings.keybinding(KeyAction::SeekBackwards),
                "General",
            ),
            help_row(
                "Seek forwards",
                self.settings.keybinding(KeyAction::SeekForwards),
                "General",
            ),
            help_row(
                "Increase volume",
                self.settings.keybinding(KeyAction::IncreaseVolume),
                "General",
            ),
            help_row(
                "Decrease volume",
                self.settings.keybinding(KeyAction::DecreaseVolume),
                "General",
            ),
            help_row(
                "Cycle repeat mode",
                self.settings.keybinding(KeyAction::Repeat),
                "General",
            ),
            help_row(
                "Toggle shuffle mode",
                self.settings.keybinding(KeyAction::Shuffle),
                "General",
            ),
            help_row(
                "Copy URL to currently playing track",
                self.settings.keybinding(KeyAction::CopySongUrl),
                "General",
            ),
            help_row(
                "Scroll down to next result page",
                self.settings.keybinding(KeyAction::NextPage),
                "Pagination",
            ),
            help_row(
                "Scroll up to previous result page",
                self.settings.keybinding(KeyAction::PreviousPage),
                "Pagination",
            ),
            help_row("Decrease sidebar width", "{", "Layout"),
            help_row("Increase sidebar width", "}", "Layout"),
            help_row("Decrease playbar or library height", "(", "Layout"),
            help_row("Increase playbar or library height", ")", "Layout"),
            help_row("Reset layout to defaults", "|", "Layout"),
            help_row(
                "Open settings menu",
                self.settings.keybinding(KeyAction::OpenSettings),
                "General",
            ),
            help_row("Search with input text", "Enter", "Search input"),
            help_row("Delete entire input", "Ctrl+l", "Search input"),
            help_row(
                "Delete text from cursor to start of input",
                "Ctrl+u",
                "Search input",
            ),
            help_row(
                "Delete text from cursor to end of input",
                "Ctrl+k",
                "Search input",
            ),
            help_row("Delete previous word", "Ctrl+w", "Search input"),
            help_row("Jump to start of input", "Ctrl+i", "Search input"),
            help_row("Jump to end of input", "Ctrl+o", "Search input"),
            help_row(
                "Escape from input back to hovered block",
                "Esc",
                "Search input",
            ),
            help_row(
                "Escape back to previously navigated pane",
                "Esc",
                "Tracks/Album/User",
            ),
            help_row("Play the selected queued track", "Enter", "Queue overlay"),
            help_row("Remove the selected queued track", "d", "Queue overlay"),
        ]
    }

    pub fn help_row_count(&self) -> usize {
        self.help_rows().len()
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn theme(&self) -> Theme {
        Theme::from_settings(&self.settings)
    }

    pub fn show_settings(&self) -> bool {
        self.settings_menu.is_some()
    }

    pub fn set_focus(&mut self, focus: Focus) {
        self.focus = focus;
    }

    pub fn focus_content_from(&mut self, focus: Focus) {
        self.content_return_focus = focus;
        self.focus = Focus::Content;
    }

    pub fn playlist_panel_title(&self) -> String {
        if self.playlists_loading {
            "Playlists (loading...)".to_string()
        } else if self.playlists_error.is_some() {
            "Playlists (error)".to_string()
        } else if self.playlists_loaded && self.playlists.is_empty() {
            "Playlists (empty)".to_string()
        } else if self.playlists_next_href.is_some() {
            format!("Playlists ({}, more available)", self.playlists.len())
        } else {
            format!("Playlists ({})", self.playlists.len())
        }
    }

    pub fn playlist_panel_placeholder(&self) -> Option<String> {
        if self.playlists_loading && self.playlists.is_empty() {
            Some("Loading playlists...".to_string())
        } else if self.playlists_error.is_some() {
            Some("Could not load playlists. Press F5 to retry.".to_string())
        } else if self.playlists_loaded && self.playlists.is_empty() {
            Some("No playlists are available for this account yet.".to_string())
        } else {
            None
        }
    }

    pub fn is_sidebar_playlist_active(&self, index: usize) -> bool {
        self.route == Route::Playlist
            && self
                .playlists
                .get(index)
                .and_then(|playlist| playlist.urn.as_deref())
                == self.active_playlist_urn.as_deref()
    }

    pub fn queue_status_label(&self) -> String {
        let queue_len = self.visible_queue_indices().len();
        let queue_position = if matches!(
            self.current_playback_plan_item().map(|item| item.kind),
            Some(PlaybackPlanItemKind::Queue)
        ) {
            format!("1/{}", queue_len.max(1))
        } else {
            format!("0/{}", queue_len)
        };

        format!(
            "Queue {} | Repeat {} | Shuffle {}",
            queue_position,
            self.player.repeat_mode.label(),
            if self.player.shuffle_enabled {
                self.settings.shuffle_icon.as_str()
            } else {
                "Off"
            }
        )
    }

    pub fn queue_overlay_rows(&self) -> Vec<ContentRow> {
        self.visible_queue_indices()
            .into_iter()
            .filter_map(|index| {
                self.playback_plan.items.get(index).map(|item| ContentRow {
                    columns: [
                        item.track.title.clone(),
                        item.track.artist.clone(),
                        self.queue_state_label_for_index(index),
                        item.track.duration_label(),
                    ],
                })
            })
            .collect()
    }

    pub fn queue_overlay_selection(&self) -> Option<usize> {
        (!self.visible_queue_indices().is_empty()).then_some(
            self.queue
                .selected
                .min(self.visible_queue_indices().len().saturating_sub(1)),
        )
    }

    pub fn can_play_next_track(&self) -> bool {
        self.next_playback_index().is_some()
    }

    pub fn can_play_previous_track(&self) -> bool {
        self.player.position_seconds > 5.0 || self.previous_playback_index().is_some()
    }

    fn current_playback_plan_item(&self) -> Option<&PlaybackPlanItem> {
        self.playback_plan
            .current_index
            .and_then(|index| self.playback_plan.items.get(index))
    }

    fn visible_queue_indices(&self) -> Vec<usize> {
        let start = match self.playback_plan.current_index {
            Some(index)
                if matches!(
                    self.playback_plan.items.get(index).map(|item| item.kind),
                    Some(PlaybackPlanItemKind::Queue)
                ) =>
            {
                index
            }
            Some(index) => index.saturating_add(1),
            None => 0,
        };

        self.playback_plan
            .items
            .iter()
            .enumerate()
            .skip(start)
            .filter_map(|(index, item)| (item.kind == PlaybackPlanItemKind::Queue).then_some(index))
            .collect()
    }

    fn pending_queue_items(&self) -> Vec<PlaybackPlanItem> {
        let start = self
            .playback_plan
            .current_index
            .map(|index| index.saturating_add(1))
            .unwrap_or(0);

        self.playback_plan
            .items
            .iter()
            .skip(start)
            .filter(|item| item.kind == PlaybackPlanItemKind::Queue)
            .cloned()
            .collect()
    }

    fn queue_state_label_for_index(&self, index: usize) -> String {
        if self.playback_plan.current_index == Some(index) {
            "Playing".to_string()
        } else if self
            .visible_queue_indices()
            .first()
            .copied()
            .is_some_and(|first| first == index)
        {
            "Next".to_string()
        } else {
            "Queued".to_string()
        }
    }

    fn build_plan_item(
        &self,
        track: TrackSummary,
        context: String,
        kind: PlaybackPlanItemKind,
    ) -> PlaybackPlanItem {
        PlaybackPlanItem {
            track,
            context,
            kind,
        }
    }

    fn rebuild_playback_plan_for_source(
        &mut self,
        tracks: Vec<TrackSummary>,
        current_index: usize,
        context: String,
    ) {
        let pending_queue = self.pending_queue_items();
        let mut items = tracks
            .into_iter()
            .map(|track| self.build_plan_item(track, context.clone(), PlaybackPlanItemKind::Source))
            .collect::<Vec<_>>();

        items.splice(
            current_index.saturating_add(1)..current_index.saturating_add(1),
            pending_queue,
        );
        self.playback_plan.items = items;
        self.playback_plan.current_index = Some(current_index);
        self.queue.selected = 0;
    }

    fn append_track_to_queue(&mut self, track: TrackSummary) {
        let item = self.build_plan_item(
            track.clone(),
            "Queue".to_string(),
            PlaybackPlanItemKind::Queue,
        );

        if let Some(current_index) = self.playback_plan.current_index {
            let mut insert_at = current_index.saturating_add(1);
            while self
                .playback_plan
                .items
                .get(insert_at)
                .is_some_and(|existing| existing.kind == PlaybackPlanItemKind::Queue)
            {
                insert_at += 1;
            }
            self.playback_plan.items.insert(insert_at, item);
        } else {
            self.playback_plan.items.push(item);
        }

        self.queue.selected = self.visible_queue_indices().len().saturating_sub(1);
        self.status = format!("Queued {}.", track.title);
    }

    pub fn help_visible_rows(&self) -> usize {
        self.viewport.height.saturating_sub(7).max(8) as usize
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

        if let Some(playlist) = self.playlists.get(playlist_index) {
            self.active_playlist_urn = playlist.urn.clone();
        }

        self.set_route(Route::Playlist);
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
            Route::Playlist => self
                .active_playlist()
                .map(|playlist| playlist.title.clone())
                .or_else(|| {
                    self.playlists
                        .get(self.selected_playlist)
                        .map(|playlist| playlist.title.clone())
                })
                .unwrap_or_else(|| "Playlist".to_string()),
            Route::UserProfile => self
                .active_user_profile
                .as_ref()
                .map(|user| user.username.clone())
                .unwrap_or_else(|| "Profile".to_string()),
            _ => self.route.label().to_string(),
        }
    }

    pub fn select_current_content(&mut self) {
        match self.current_selected_content() {
            Some(SelectedContent::Track { track, context }) => {
                if let Some((tracks, current_index)) = self.current_track_queue_selection() {
                    self.rebuild_playback_plan_for_source(tracks, current_index, context.clone());
                } else {
                    self.rebuild_playback_plan_for_source(vec![track.clone()], 0, context.clone());
                }
                self.start_track_playback(track, context);
            }
            Some(SelectedContent::Playlist(playlist)) => {
                self.open_playlist(playlist);
            }
            Some(SelectedContent::User(user)) => {
                self.open_user_profile(user);
            }
            None => {
                let Some(row) = self.current_content_row() else {
                    self.status = "Nothing selected in the content pane.".to_string();
                    return;
                };

                if self.route.is_track_view() {
                    self.status = format!(
                        "Playback is unavailable until SoundCloud authentication is complete for {}.",
                        row.columns[0]
                    );
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
            artwork_url: track.artwork_url.clone(),
            elapsed_label: "0:00".to_string(),
            duration_label: track.duration_label(),
            progress_ratio: 0.0,
        };
        self.refresh_cover_art(track.artwork_url.as_deref());
        self.status = format!("Resolving SoundCloud stream for {}...", track.title);
        self.queue_command(AppCommand::PlayTrack { session, track });
    }

    fn refresh_cover_art(&mut self, artwork_url: Option<&str>) {
        let Some(artwork_url) = artwork_url.map(str::trim).filter(|value| !value.is_empty()) else {
            self.cover_art = CoverArt::default();
            return;
        };

        if self.cover_art.url.as_deref() == Some(artwork_url) {
            return;
        }

        self.cover_art.url = Some(artwork_url.to_string());
        self.cover_art.bytes = None;
        self.cover_art.loading = true;
        self.queue_command(AppCommand::LoadCoverArt {
            url: artwork_url.to_string(),
        });
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
            Route::RecentlyPlayed => Some((
                self.recent_history
                    .entries
                    .iter()
                    .map(|entry| entry.track.clone())
                    .collect(),
                self.selected_content,
            )),
            Route::Playlist => {
                let urn = self.active_playlist_urn.as_ref()?;
                let tracks = self.playlist_tracks.get(urn)?.items.clone();
                Some((tracks, self.selected_content))
            }
            Route::Search if self.search_view == SearchView::Tracks => {
                Some((self.search_tracks.items.clone(), self.selected_content))
            }
            Route::UserProfile if self.user_profile_view == UserProfileView::Tracks => Some((
                self.user_profile_tracks.items.clone(),
                self.selected_content,
            )),
            Route::Search => None,
            Route::Albums | Route::Following | Route::UserProfile => None,
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
        let Some(next_index) = self.next_playback_index() else {
            self.status = "Reached the end of the queue.".to_string();
            return;
        };

        let Some(item) = self.playback_plan.items.get(next_index).cloned() else {
            self.status = "Reached the end of the queue.".to_string();
            return;
        };

        self.playback_plan.current_index = Some(next_index);
        self.start_track_playback(item.track, item.context);
    }

    fn next_playback_index(&self) -> Option<usize> {
        let current_index = self.playback_plan.current_index?;
        let track_count = self.playback_plan.items.len();

        if current_index + 1 < track_count {
            Some(current_index + 1)
        } else if self.player.repeat_mode == RepeatMode::Queue && track_count > 0 {
            Some(0)
        } else {
            None
        }
    }

    fn previous_playback_index(&self) -> Option<usize> {
        let current_index = self.playback_plan.current_index?;
        let track_count = self.playback_plan.items.len();

        if current_index > 0 {
            Some(current_index - 1)
        } else if self.player.repeat_mode == RepeatMode::Queue && track_count > 0 {
            Some(track_count.saturating_sub(1))
        } else {
            None
        }
    }

    fn restart_current_track(&mut self) -> bool {
        let track = self
            .current_playback_plan_item()
            .cloned()
            .map(|item| (item.track, item.context))
            .or_else(|| {
                self.now_playing
                    .track
                    .clone()
                    .map(|track| (track, self.now_playing.context.clone()))
            });

        let Some((track, context)) = track else {
            self.status = "Nothing queued for playback.".to_string();
            return false;
        };

        self.start_track_playback(track, context);
        true
    }

    fn play_previous_track(&mut self) {
        if self.playback_plan.current_index.is_none() {
            self.status = "Nothing queued for playback.".to_string();
            return;
        }

        if self.player.position_seconds > 5.0 {
            self.queue_command(AppCommand::ControlPlayback(PlayerCommand::SeekAbsolute {
                seconds: 0.0,
            }));
            self.status = "Restarting the current track.".to_string();
            return;
        }

        let Some(previous_index) = self.previous_playback_index() else {
            self.status = "Already at the start of the queue.".to_string();
            return;
        };

        let Some(item) = self.playback_plan.items.get(previous_index).cloned() else {
            self.status = "Already at the start of the queue.".to_string();
            return;
        };

        self.playback_plan.current_index = Some(previous_index);
        self.start_track_playback(item.track, item.context);
    }

    fn force_previous_track(&mut self) {
        if self.playback_plan.current_index.is_none() {
            self.status = "Nothing queued for playback.".to_string();
            return;
        }

        if let Some(previous_index) = self.previous_playback_index() {
            if let Some(item) = self.playback_plan.items.get(previous_index).cloned() {
                self.playback_plan.current_index = Some(previous_index);
                self.start_track_playback(item.track, item.context);
                return;
            }
        }

        let _ = self.restart_current_track();
    }

    fn apply_player_event(&mut self, event: PlayerEvent) {
        match event {
            PlayerEvent::PlaybackStarted => {
                self.record_recent_playback();
                self.player.status = PlaybackStatus::Playing;
                if let Some(track) = &self.now_playing.track {
                    self.status = format!("Playing {}.", track.title);
                }
                self.sync_window_title();
            }
            PlayerEvent::PlaybackResumed => {
                self.player.status = PlaybackStatus::Playing;
                if let Some(track) = &self.now_playing.track {
                    self.status = format!("Playing {}.", track.title);
                }
                self.sync_window_title();
            }
            PlayerEvent::PlaybackPaused => {
                self.player.status = PlaybackStatus::Paused;
                if let Some(track) = &self.now_playing.track {
                    self.status = format!("Paused {}.", track.title);
                }
                self.sync_window_title();
            }
            PlayerEvent::PlaybackStopped => {
                self.player.status = PlaybackStatus::Stopped;
                self.player.position_seconds = 0.0;
                self.now_playing.elapsed_label = "0:00".to_string();
                self.now_playing.progress_ratio = 0.0;
                self.status = "Playback stopped.".to_string();
                self.sync_window_title();
            }
            PlayerEvent::TrackEnded => {
                self.player.status = PlaybackStatus::Stopped;
                self.player.position_seconds = 0.0;
                self.now_playing.elapsed_label = "0:00".to_string();
                self.now_playing.progress_ratio = 0.0;
                if self.settings.stop_after_current_track {
                    self.status = "Stopped after the current track.".to_string();
                } else if self.player.repeat_mode == RepeatMode::Track {
                    if !self.restart_current_track() {
                        self.status = "Reached the end of the queue.".to_string();
                    }
                } else if self.next_playback_index().is_some() {
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
                self.show_main_error("Playback backend error", error);
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
                self.queue_command(AppCommand::LoadPlaylistTracks {
                    session,
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
                        self.queue_command(AppCommand::LoadUserTracks {
                            session,
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
                        self.queue_command(AppCommand::LoadUserPlaylists {
                            session,
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

        if self
            .toast
            .as_ref()
            .is_some_and(|toast| self.tick_count >= toast.expires_at_tick)
        {
            self.toast = None;
        }

        if let Some(loading) = &mut self.loading {
            loading.ticks_remaining = loading.ticks_remaining.saturating_sub(1);
            if loading.ticks_remaining == 0 {
                self.loading = None;
            }
        }
    }

    pub fn on_resize(&mut self, width: u16, height: u16) {
        self.viewport = Viewport { width, height };
        self.help_scroll = self.max_help_scroll().min(self.help_scroll);
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

    fn show_error_modal(&mut self, title: impl Into<String>, message: impl Into<String>) {
        self.add_to_playlist_modal = None;
        self.error_modal = Some(ErrorModal {
            title: title.into(),
            message: message.into(),
        });
    }

    fn show_main_error(&mut self, title: impl Into<String>, message: impl Into<String>) {
        let title = title.into();
        self.show_error_modal(title.clone(), message);
        self.status = title;
    }

    fn show_toast(&mut self, message: impl Into<String>) {
        self.toast = Some(Toast {
            message: message.into(),
            expires_at_tick: self.tick_count.saturating_add(12),
        });
    }

    fn dismiss_error_modal(&mut self) {
        self.error_modal = None;
        self.status = "Dismissed the latest error.".to_string();
    }

    fn handle_error_modal_key(&mut self, key: KeyEvent) {
        if matches!(key.code, KeyCode::Esc | KeyCode::Enter)
            || self.settings.key_matches(KeyAction::Back, key)
        {
            self.dismiss_error_modal();
        }
    }

    fn dismiss_add_to_playlist_modal(&mut self) {
        self.add_to_playlist_modal = None;
        self.status = "Cancelled add to playlist.".to_string();
    }

    fn handle_add_to_playlist_modal_key(&mut self, key: KeyEvent) {
        match (key.code, key.modifiers) {
            _ if matches!(key.code, KeyCode::Esc)
                || self.settings.key_matches(KeyAction::Back, key) =>
            {
                self.dismiss_add_to_playlist_modal()
            }
            (KeyCode::Enter, _) => self.confirm_add_to_playlist_selection(),
            (KeyCode::Down, _) | (KeyCode::Char('j'), KeyModifiers::NONE) => {
                self.move_add_to_playlist_selection(1)
            }
            (KeyCode::Up, _) | (KeyCode::Char('k'), KeyModifiers::NONE) => {
                self.move_add_to_playlist_selection(-1)
            }
            (KeyCode::Char('H'), _) => self.jump_add_to_playlist_selection(0),
            (KeyCode::Char('M'), _) => {
                self.jump_add_to_playlist_selection(self.playlists.len().saturating_sub(1) / 2)
            }
            (KeyCode::Char('L'), _) => {
                self.jump_add_to_playlist_selection(self.playlists.len().saturating_sub(1))
            }
            _ => {}
        }
    }

    fn move_add_to_playlist_selection(&mut self, delta: isize) {
        let Some(current) = self
            .add_to_playlist_modal
            .as_ref()
            .map(|modal| modal.selected_playlist)
        else {
            return;
        };

        if self.playlists.is_empty() {
            self.status = "No playlists are available yet.".to_string();
            return;
        }

        let max_index = self.playlists.len().saturating_sub(1);
        let next = (current as isize + delta).clamp(0, max_index as isize) as usize;
        if let Some(modal) = self.add_to_playlist_modal.as_mut() {
            modal.selected_playlist = next;
        }

        if next == current && delta > 0 {
            let _ = self.maybe_queue_more_playlists();
        }

        if let Some(playlist) = self.playlists.get(next) {
            self.status = format!("Selected playlist {}.", playlist.title);
        }
    }

    fn jump_add_to_playlist_selection(&mut self, index: usize) {
        if self.playlists.is_empty() {
            self.status = "No playlists are available yet.".to_string();
            return;
        }

        let next = index.min(self.playlists.len().saturating_sub(1));
        if let Some(modal) = self.add_to_playlist_modal.as_mut() {
            modal.selected_playlist = next;
        }

        if let Some(playlist) = self.playlists.get(next) {
            self.status = format!("Selected playlist {}.", playlist.title);
        }
    }

    fn confirm_add_to_playlist_selection(&mut self) {
        let Some(modal) = self.add_to_playlist_modal.clone() else {
            return;
        };

        let Some(session) = self.session.clone() else {
            self.dismiss_add_to_playlist_modal();
            return;
        };

        let Some(playlist_urn) = self
            .playlists
            .get(modal.selected_playlist)
            .and_then(|playlist| playlist.urn.as_deref())
        else {
            self.status = "Select a playlist first.".to_string();
            return;
        };
        let Some(playlist) = self.known_playlists.get(playlist_urn).cloned() else {
            self.status = "The selected playlist details are not available yet.".to_string();
            return;
        };

        self.add_to_playlist_modal = None;
        self.status = format!("Adding {} to {}...", modal.track.title, playlist.title);
        self.queue_command(AppCommand::AddTrackToPlaylist {
            session,
            track: modal.track,
            playlist,
        });
    }

    fn max_help_scroll(&self) -> usize {
        self.help_row_count()
            .saturating_sub(self.help_visible_rows())
    }

    fn scroll_help(&mut self, delta: isize) {
        let next = self.help_scroll as isize + delta;
        self.help_scroll = next.clamp(0, self.max_help_scroll() as isize) as usize;
    }

    fn page_help(&mut self, down: bool) {
        let step = self.help_visible_rows().max(1) as isize;
        self.scroll_help(if down { step } else { -step });
    }

    fn content_page_size(&self) -> usize {
        self.viewport
            .height
            .saturating_sub(self.layout.playbar_height + 8)
            .max(6) as usize
    }

    fn playlists_page_size(&self) -> usize {
        self.viewport
            .height
            .saturating_sub(self.layout.playbar_height + self.layout.library_height + 8)
            .max(4) as usize
    }

    fn page_results(&mut self, down: bool) -> bool {
        match self.focus {
            Focus::Content => self.page_content(down),
            Focus::Playlists => self.page_playlists(down),
            _ => false,
        }
    }

    fn page_content(&mut self, down: bool) -> bool {
        let len = self.current_content_len();
        if len == 0 {
            return down && self.maybe_queue_current_route_next_page();
        }

        let step = self.content_page_size();
        let max_index = len.saturating_sub(1);
        let next = if down {
            self.selected_content.saturating_add(step).min(max_index)
        } else {
            self.selected_content.saturating_sub(step)
        };
        let moved = next != self.selected_content;
        self.selected_content = next;

        if moved {
            if let Some(label) = self.current_selection_label() {
                self.status = format!("Highlighted {}.", label);
            }
        }

        let queued_more = down
            && self.selected_content == max_index
            && self.maybe_queue_current_route_next_page();
        moved || queued_more
    }

    fn page_playlists(&mut self, down: bool) -> bool {
        if self.playlists.is_empty() {
            return down && self.maybe_queue_more_playlists();
        }

        let step = self.playlists_page_size();
        let max_index = self.playlists.len().saturating_sub(1);
        let next = if down {
            self.selected_playlist.saturating_add(step).min(max_index)
        } else {
            self.selected_playlist.saturating_sub(step)
        };
        let moved = next != self.selected_playlist;
        self.selected_playlist = next;

        if moved {
            self.sync_route_from_playlist();
        }

        let queued_more =
            down && self.selected_playlist == max_index && self.maybe_queue_more_playlists();
        moved || queued_more
    }

    fn open_settings_menu(&mut self) {
        self.show_help = false;
        self.settings_menu = Some(SettingsMenuState::new(&self.settings));
        self.status = "Opened settings.".to_string();
    }

    fn close_settings_menu(&mut self) {
        let discarded = self
            .settings_menu
            .as_ref()
            .map(|menu| menu.has_unsaved_changes(&self.settings))
            .unwrap_or(false);
        self.settings_menu = None;
        self.status = if discarded {
            "Discarded unsaved settings changes.".to_string()
        } else {
            "Closed settings.".to_string()
        };
    }

    fn save_settings_menu(&mut self) {
        let Some(mut menu) = self.settings_menu.take() else {
            return;
        };

        let previous = self.settings.clone();
        menu.draft.normalize();
        if let Err(error) = menu.draft.validate() {
            self.show_main_error("Could not save settings", error.to_string());
            self.settings_menu = Some(menu);
            return;
        }

        self.settings = menu.draft.clone();
        self.queue_command(AppCommand::SaveSettings(self.settings.clone()));
        self.apply_runtime_settings(&previous);
        menu.draft = self.settings.clone();
        menu.editing = false;
        menu.edit_buffer.clear();
        self.settings_menu = Some(menu);

        let restart_note = if previous.tick_rate_ms != self.settings.tick_rate_ms {
            " Tick rate applies on the next launch."
        } else {
            ""
        };
        self.status = format!("Saved settings.{}", restart_note);
    }

    fn handle_settings_key(&mut self, key: KeyEvent) {
        let Some(mut menu) = self.settings_menu.take() else {
            return;
        };

        if menu.editing {
            let selected = menu.items().get(menu.selected_index()).cloned();
            if matches!(key.code, KeyCode::Esc) {
                menu.cancel_edit();
                self.status = "Cancelled the current settings edit.".to_string();
                self.settings_menu = Some(menu);
                return;
            }

            match selected.map(|item| item.value) {
                Some(crate::app::SettingsValue::Key(_)) => match menu.capture_keybinding(key) {
                    Ok(binding) => {
                        self.status = format!("Bound setting to {}.", binding);
                    }
                    Err(error) => {
                        self.show_main_error("Could not update keybinding", error.to_string());
                    }
                },
                Some(crate::app::SettingsValue::Number(_))
                | Some(crate::app::SettingsValue::Text(_))
                | Some(crate::app::SettingsValue::Color(_)) => match (key.code, key.modifiers) {
                    (KeyCode::Enter, _) => match menu.confirm_edit() {
                        Ok(()) => self.status = "Updated the draft setting value.".to_string(),
                        Err(error) => {
                            self.show_main_error("Could not update setting", error.to_string())
                        }
                    },
                    (KeyCode::Backspace, _) => {
                        menu.edit_buffer.pop();
                    }
                    (KeyCode::Char(ch), modifiers)
                        if modifiers.intersection(KeyModifiers::CONTROL | KeyModifiers::ALT)
                            == KeyModifiers::NONE =>
                    {
                        menu.edit_buffer.push(ch);
                    }
                    _ => {}
                },
                _ => {}
            }

            self.settings_menu = Some(menu);
            return;
        }

        if self.settings.key_matches(KeyAction::SaveSettings, key) {
            self.settings_menu = Some(menu);
            self.save_settings_menu();
            return;
        }

        if matches!(key.code, KeyCode::Esc) || self.settings.key_matches(KeyAction::Back, key) {
            self.settings_menu = Some(menu);
            self.close_settings_menu();
            return;
        }

        match (key.code, key.modifiers) {
            (KeyCode::Left, _) => menu.switch_tab(false),
            (KeyCode::Right, _) => menu.switch_tab(true),
            (KeyCode::Down, _) | (KeyCode::Char('j'), KeyModifiers::NONE) => menu.move_selection(1),
            (KeyCode::Up, _) | (KeyCode::Char('k'), KeyModifiers::NONE) => menu.move_selection(-1),
            (KeyCode::Enter, _) => match menu.activate_selected() {
                Ok(_) => {
                    self.status = format!("Editing {} settings.", menu.tab.label());
                }
                Err(error) => self.show_main_error("Could not update setting", error.to_string()),
            },
            _ => {}
        }

        self.settings_menu = Some(menu);
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
                if self.show_help {
                    self.handle_help_key(key);
                    return;
                }

                if self.error_modal.is_some() {
                    self.handle_error_modal_key(key);
                    return;
                }

                if self.settings_menu.is_some() {
                    self.handle_settings_key(key);
                    return;
                }

                if self.add_to_playlist_modal.is_some() {
                    self.handle_add_to_playlist_modal_key(key);
                    return;
                }

                if self.queue.overlay_visible {
                    self.handle_queue_key(key);
                    return;
                }

                if self.show_welcome {
                    self.show_welcome = false;
                }

                if self.focus == Focus::Search && self.handle_search_key(key) {
                    return;
                }

                if self.handle_main_shortcut_key(key) {
                    return;
                }

                if self.handle_route_key(key) {
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

    fn handle_help_key(&mut self, key: KeyEvent) {
        if matches!(key.code, KeyCode::Esc | KeyCode::Enter | KeyCode::F(1))
            || self.settings.key_matches(KeyAction::Help, key)
            || self.settings.key_matches(KeyAction::Back, key)
        {
            self.dismiss_help();
            return;
        }

        match (key.code, key.modifiers) {
            (KeyCode::Down, _) | (KeyCode::Char('j'), KeyModifiers::NONE) => self.scroll_help(1),
            (KeyCode::Up, _) | (KeyCode::Char('k'), KeyModifiers::NONE) => self.scroll_help(-1),
            _ if self.settings.key_matches(KeyAction::NextPage, key) => self.page_help(true),
            _ if self.settings.key_matches(KeyAction::PreviousPage, key) => self.page_help(false),
            _ => {}
        }
    }

    fn handle_queue_key(&mut self, key: KeyEvent) {
        match (key.code, key.modifiers) {
            _ if matches!(key.code, KeyCode::Esc)
                || self.settings.key_matches(KeyAction::Back, key) =>
            {
                self.close_queue_overlay()
            }
            (KeyCode::Enter, _) => self.play_selected_queue_track(),
            (KeyCode::Down, _) | (KeyCode::Char('j'), KeyModifiers::NONE) => {
                self.move_queue_selection(true)
            }
            (KeyCode::Up, _) | (KeyCode::Char('k'), KeyModifiers::NONE) => {
                self.move_queue_selection(false)
            }
            (KeyCode::Char('d'), KeyModifiers::NONE) => self.remove_selected_queue_track(),
            _ => {}
        }
    }

    fn handle_main_shortcut_key(&mut self, key: KeyEvent) -> bool {
        if self.settings.key_matches(KeyAction::Search, key) {
            self.begin_search_input();
            return true;
        }

        if self.settings.key_matches(KeyAction::AddToQueue, key) && self.focus == Focus::Content {
            self.queue_selected_track();
            return true;
        }

        match (key.code, key.modifiers) {
            (KeyCode::Char('w'), KeyModifiers::NONE) if self.focus == Focus::Content => {
                self.open_add_to_playlist_modal_for_selected_track();
                true
            }
            (KeyCode::Char('l'), KeyModifiers::NONE) if self.focus == Focus::Content => {
                self.like_selected_track();
                true
            }
            (KeyCode::Char('W'), KeyModifiers::SHIFT) => {
                self.open_add_to_playlist_modal_for_now_playing();
                true
            }
            (KeyCode::Char('L'), KeyModifiers::SHIFT) => {
                self.like_now_playing_track();
                true
            }
            (KeyCode::F(1), _) => {
                self.help_scroll = 0;
                self.show_help = true;
                self.status = "Showing help menu.".to_string();
                true
            }
            (KeyCode::Esc, _) if self.focus == Focus::Content => {
                self.focus = self.content_return_focus;
                self.status = format!("Returned focus to {}.", self.focus.label());
                true
            }
            (KeyCode::Char('{'), _) => {
                self.adjust_sidebar_width(-2);
                true
            }
            (KeyCode::Char('}'), _) => {
                self.adjust_sidebar_width(2);
                true
            }
            (KeyCode::Char('('), _) => {
                self.adjust_primary_panel_height(-1);
                true
            }
            (KeyCode::Char(')'), _) => {
                self.adjust_primary_panel_height(1);
                true
            }
            (KeyCode::Char('|'), _) => {
                self.reset_layout();
                true
            }
            (KeyCode::F(5), _) => {
                self.reload_current_route();
                true
            }
            _ if self.settings.key_matches(KeyAction::ShowQueue, key) => {
                self.open_queue_overlay();
                true
            }
            _ if self.settings.key_matches(KeyAction::Help, key) => {
                self.help_scroll = 0;
                self.show_help = true;
                self.status = "Showing help menu.".to_string();
                true
            }
            _ if self.settings.key_matches(KeyAction::OpenSettings, key) => {
                self.open_settings_menu();
                true
            }
            _ if self.settings.key_matches(KeyAction::NextPage, key) => self.page_results(true),
            _ if self.settings.key_matches(KeyAction::PreviousPage, key) => {
                self.page_results(false)
            }
            _ if self.settings.key_matches(KeyAction::Repeat, key) => {
                self.cycle_repeat_mode();
                true
            }
            _ if self.settings.key_matches(KeyAction::Shuffle, key) => {
                self.apply_playback_intent(PlaybackIntent::SetShuffle(
                    !self.player.shuffle_enabled,
                ));
                true
            }
            _ if self.settings.key_matches(KeyAction::CopySongUrl, key) => {
                self.copy_now_playing_url();
                true
            }
            _ => false,
        }
    }

    fn handle_route_key(&mut self, key: KeyEvent) -> bool {
        match self.route {
            Route::Search => match key.code {
                KeyCode::Char('1') => {
                    self.set_search_view(SearchView::Tracks);
                    true
                }
                KeyCode::Char('2') => {
                    self.set_search_view(SearchView::Playlists);
                    true
                }
                KeyCode::Char('3') => {
                    self.set_search_view(SearchView::Users);
                    true
                }
                _ => false,
            },
            Route::UserProfile => match key.code {
                KeyCode::Char('1') => {
                    self.set_user_profile_view(UserProfileView::Tracks);
                    true
                }
                KeyCode::Char('2') => {
                    self.set_user_profile_view(UserProfileView::Playlists);
                    true
                }
                _ => false,
            },
            _ => false,
        }
    }

    fn handle_paste_event(&mut self, text: &str) {
        match self.mode {
            AppMode::Auth => {
                self.auth.paste_text(text);
                self.status = "Pasted clipboard contents into the active field.".to_string();
            }
            AppMode::Main if self.settings_menu.as_ref().is_some_and(|menu| menu.editing) => {
                if let Some(menu) = self.settings_menu.as_mut() {
                    let sanitized = text.replace(['\r', '\n'], " ");
                    menu.edit_buffer.push_str(sanitized.trim());
                    self.status = "Pasted text into the current settings field.".to_string();
                }
            }
            AppMode::Main if self.focus == Focus::Search => {
                self.show_welcome = false;
                let sanitized = text.replace(['\r', '\n'], " ");
                self.insert_search_text(sanitized.trim());
                self.status = format!("Updated search query to '{}'.", self.search_query);
            }
            AppMode::Main => {
                self.show_welcome = false;
            }
        }
    }

    fn handle_search_key(&mut self, key: KeyEvent) -> bool {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => {
                self.focus = self.search_return_focus;
                self.status = "Closed search input.".to_string();
                true
            }
            (KeyCode::Enter, _) => {
                self.submit_search();
                true
            }
            (KeyCode::Left, _) => {
                self.search_cursor = self.search_cursor.saturating_sub(1);
                true
            }
            (KeyCode::Right, _) => {
                self.search_cursor =
                    (self.search_cursor + 1).min(self.search_query.chars().count());
                true
            }
            (KeyCode::Home, _) | (KeyCode::Char('i'), KeyModifiers::CONTROL) => {
                self.search_cursor = 0;
                true
            }
            (KeyCode::End, _) | (KeyCode::Char('o'), KeyModifiers::CONTROL) => {
                self.search_cursor = self.search_query.chars().count();
                true
            }
            (KeyCode::Backspace, _) => {
                self.backspace_search();
                true
            }
            (KeyCode::Delete, _) => {
                self.delete_search();
                true
            }
            (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
                self.search_query.clear();
                self.search_cursor = 0;
                self.status = "Cleared the search query.".to_string();
                true
            }
            (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                self.delete_search_to_start();
                true
            }
            (KeyCode::Char('k'), KeyModifiers::CONTROL) => {
                self.delete_search_to_end();
                true
            }
            (KeyCode::Char('w'), KeyModifiers::CONTROL) => {
                self.delete_previous_word();
                true
            }
            (KeyCode::Char(ch), KeyModifiers::NONE) | (KeyCode::Char(ch), KeyModifiers::SHIFT) => {
                self.insert_search_char(ch);
                true
            }
            _ => false,
        }
    }

    fn begin_search_input(&mut self) {
        if self.focus != Focus::Search {
            self.search_return_focus = self.focus;
        }
        self.set_focus(Focus::Search);
        self.search_cursor = self.search_query.chars().count();
        self.status = "Editing search query. Press Enter to search.".to_string();
    }

    fn selected_track_shortcut_target(&self) -> Option<TrackSummary> {
        if self.focus != Focus::Content {
            return None;
        }

        match self.current_selected_content() {
            Some(SelectedContent::Track { track, .. }) => Some(track),
            _ => None,
        }
    }

    fn now_playing_shortcut_target(&self) -> Option<TrackSummary> {
        self.now_playing.track.clone()
    }

    fn open_queue_overlay(&mut self) {
        self.queue.overlay_visible = true;
        self.queue.selected = self
            .queue_overlay_selection()
            .unwrap_or(0)
            .min(self.visible_queue_indices().len().saturating_sub(1));
        self.status = if self.visible_queue_indices().is_empty() {
            "Queue is empty.".to_string()
        } else {
            "Opened queue overlay.".to_string()
        };
    }

    fn close_queue_overlay(&mut self) {
        self.queue.overlay_visible = false;
        self.status = "Closed queue overlay.".to_string();
    }

    fn move_queue_selection(&mut self, down: bool) {
        let len = self.visible_queue_indices().len();
        if len == 0 {
            self.status = "Queue is empty.".to_string();
            return;
        }

        if down {
            self.queue.selected = self
                .queue
                .selected
                .saturating_add(1)
                .min(len.saturating_sub(1));
        } else {
            self.queue.selected = self.queue.selected.saturating_sub(1);
        }

        if let Some(row) = self
            .queue_overlay_rows()
            .get(self.queue.selected.min(len.saturating_sub(1)))
        {
            self.status = format!("Queued {} highlighted.", row.columns[0]);
        }
    }

    fn queue_selected_track(&mut self) {
        let Some(track) = self.selected_track_shortcut_target() else {
            self.status = "Select a track first.".to_string();
            return;
        };

        self.append_track_to_queue(track);
    }

    fn play_selected_queue_track(&mut self) {
        let indices = self.visible_queue_indices();
        let Some(plan_index) = indices
            .get(self.queue.selected.min(indices.len().saturating_sub(1)))
            .copied()
        else {
            self.status = "Queue is empty.".to_string();
            return;
        };

        let Some(item) = self.playback_plan.items.get(plan_index).cloned() else {
            self.status = "Queue is empty.".to_string();
            return;
        };

        self.playback_plan.current_index = Some(plan_index);
        self.start_track_playback(item.track, item.context);
    }

    fn remove_selected_queue_track(&mut self) {
        let indices = self.visible_queue_indices();
        let Some(plan_index) = indices
            .get(self.queue.selected.min(indices.len().saturating_sub(1)))
            .copied()
        else {
            self.status = "Queue is empty.".to_string();
            return;
        };

        if self.playback_plan.current_index == Some(plan_index) {
            self.status = "Can't remove the currently playing queued track.".to_string();
            return;
        }

        let removed = self.playback_plan.items.remove(plan_index);
        if let Some(current_index) = self.playback_plan.current_index.as_mut() {
            if plan_index < *current_index {
                *current_index = (*current_index).saturating_sub(1);
            }
        }

        let len = self.visible_queue_indices().len();
        self.queue.selected = self.queue.selected.min(len.saturating_sub(1));
        self.status = format!("Removed {} from the queue.", removed.track.title);
    }

    fn open_add_to_playlist_modal_for_selected_track(&mut self) {
        let track = self.selected_track_shortcut_target();
        self.open_add_to_playlist_modal(track, "Select a track first.");
    }

    fn open_add_to_playlist_modal_for_now_playing(&mut self) {
        let track = self.now_playing_shortcut_target();
        self.open_add_to_playlist_modal(track, "Nothing is playing right now.");
    }

    fn open_add_to_playlist_modal(
        &mut self,
        track: Option<TrackSummary>,
        missing_track_message: &str,
    ) {
        let Some(track) = track else {
            self.status = missing_track_message.to_string();
            return;
        };

        if self.session.is_none() {
            self.status = "Connect to SoundCloud before editing playlists.".to_string();
            return;
        }

        if self.playlists_loading && self.playlists.is_empty() {
            self.status = "Playlists are still loading. Try again in a moment.".to_string();
            return;
        }

        if self.playlists.is_empty() {
            if !self.playlists_loaded || self.playlists_error.is_some() {
                self.invalidate_playlists_sidebar();
                self.status = "Loading playlists before opening the playlist picker...".to_string();
            } else {
                self.status = "No playlists are available for this account yet.".to_string();
            }
            return;
        }

        self.add_to_playlist_modal = Some(AddToPlaylistModal {
            track: track.clone(),
            selected_playlist: self
                .selected_playlist
                .min(self.playlists.len().saturating_sub(1)),
        });
        self.status = format!("Choose a playlist for {}.", track.title);
    }

    fn like_selected_track(&mut self) {
        let track = self.selected_track_shortcut_target();
        self.like_track(track, "Select a track first.");
    }

    fn like_now_playing_track(&mut self) {
        let track = self.now_playing_shortcut_target();
        self.like_track(track, "Nothing is playing right now.");
    }

    fn like_track(&mut self, track: Option<TrackSummary>, missing_track_message: &str) {
        let Some(track) = track else {
            self.status = missing_track_message.to_string();
            return;
        };

        let Some(session) = self.session.clone() else {
            self.status = "Connect to SoundCloud before liking tracks.".to_string();
            return;
        };

        self.status = format!("Adding {} to Liked Songs...", track.title);
        self.queue_command(AppCommand::LikeTrack { session, track });
    }

    fn handle_playback_key(&mut self, key: KeyEvent) -> bool {
        let seek_seconds = self.settings.seek_duration_ms as f64 / 1000.0;
        let volume_increment = self.settings.volume_increment as f64;

        if self.settings.key_matches(KeyAction::TogglePlayback, key) {
            self.apply_playback_intent(PlaybackIntent::TogglePause);
            return true;
        }

        if self.settings.key_matches(KeyAction::NextTrack, key) {
            self.apply_playback_intent(PlaybackIntent::Next);
            return true;
        }

        if self.settings.key_matches(KeyAction::PreviousTrack, key) {
            self.apply_playback_intent(PlaybackIntent::Previous);
            return true;
        }

        if self
            .settings
            .key_matches(KeyAction::ForcePreviousTrack, key)
        {
            self.force_previous_track();
            return true;
        }

        if self.settings.key_matches(KeyAction::SeekBackwards, key) {
            self.apply_playback_intent(PlaybackIntent::SeekRelative {
                seconds: -seek_seconds,
            });
            return true;
        }

        if self.settings.key_matches(KeyAction::SeekForwards, key) {
            self.apply_playback_intent(PlaybackIntent::SeekRelative {
                seconds: seek_seconds,
            });
            return true;
        }

        if self.settings.key_matches(KeyAction::DecreaseVolume, key) {
            self.apply_playback_intent(PlaybackIntent::SetVolume {
                percent: self.player.volume_percent - volume_increment,
            });
            return true;
        }

        if self.settings.key_matches(KeyAction::IncreaseVolume, key) {
            self.apply_playback_intent(PlaybackIntent::SetVolume {
                percent: self.player.volume_percent + volume_increment,
            });
            return true;
        }

        false
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
        self.apply_startup_behavior();
        self.show_welcome = !self.settings.show_help_on_startup;
        if self.settings.show_help_on_startup {
            self.help_scroll = 0;
            self.show_help = true;
            self.help_requires_acknowledgement = true;
            self.status =
                "Connected successfully. Review the help menu for first-run guidance.".to_string();
        }
        self.request_playlists_load(false);
        self.request_route_load(false);
        self.sync_window_title();
    }

    fn apply_runtime_settings(&mut self, previous: &Settings) {
        if !self.settings.draw_cover_art {
            self.cover_art = CoverArt::default();
        }

        if previous.startup_behavior != self.settings.startup_behavior && self.session.is_some() {
            self.apply_startup_behavior();
        }

        self.sync_window_title();
    }

    fn apply_startup_behavior(&mut self) {
        let Some(track) = self
            .recent_history
            .entries
            .first()
            .map(|entry| entry.track.clone())
        else {
            return;
        };

        match self.settings.startup_behavior {
            StartupBehavior::Continue => {}
            StartupBehavior::Pause => {
                self.now_playing = NowPlaying {
                    track: Some(track.clone()),
                    title: track.title.clone(),
                    artist: track.artist.clone(),
                    context: "Startup: recent track".to_string(),
                    artwork_url: track.artwork_url.clone(),
                    elapsed_label: "0:00".to_string(),
                    duration_label: track.duration_label(),
                    progress_ratio: 0.0,
                };
                self.refresh_cover_art(track.artwork_url.as_deref());
                self.status = format!("Loaded {} into the playbar.", track.title);
            }
            StartupBehavior::Play => {
                self.now_playing.context = "Startup: recent track".to_string();
                self.start_track_playback(track.clone(), "Startup: recent track".to_string());
                self.status = format!("Starting your most recent track: {}.", track.title);
            }
        }
    }

    fn queue_command(&mut self, command: AppCommand) {
        self.pending_commands.push(command);
    }

    fn sync_window_title(&mut self) {
        let title = if self.settings.set_window_title {
            match self.now_playing.track.as_ref() {
                Some(track) => format!("{} - {} | soundcloud-tui", track.title, track.artist),
                None => format!("{} | soundcloud-tui", self.route_title()),
            }
        } else {
            "soundcloud-tui".to_string()
        };

        self.queue_command(AppCommand::SetWindowTitle(title));
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
                self.queue_command(AppCommand::LoadPlaylistTracks {
                    session,
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
                        self.queue_command(AppCommand::LoadUserTracks {
                            session,
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
                        self.queue_command(AppCommand::LoadUserPlaylists {
                            session,
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
                self.queue_command(AppCommand::SearchAll {
                    session,
                    query: self.search_query.clone(),
                });
            }
        }
    }

    fn invalidate_liked_tracks(&mut self) {
        self.liked_tracks = CollectionState::default();
        if self.route == Route::LikedSongs {
            self.request_route_load(false);
        }
    }

    fn invalidate_playlists_sidebar(&mut self) {
        self.playlists_loading = false;
        self.playlists_loaded = false;
        self.playlists_error = None;
        self.playlists_next_href = None;
        self.playlists.clear();
        self.request_playlists_load(false);
    }

    fn invalidate_playlist_tracks(&mut self, playlist_urn: &str) {
        self.playlist_tracks
            .insert(playlist_urn.to_string(), CollectionState::default());

        if self.active_playlist_urn.as_deref() == Some(playlist_urn)
            && self.route == Route::Playlist
        {
            self.request_route_load(false);
        }
    }

    fn bump_playlist_track_count(&mut self, playlist_urn: &str) {
        if let Some(playlist) = self.known_playlists.get_mut(playlist_urn) {
            playlist.track_count = playlist.track_count.saturating_add(1);
        }
    }

    fn reset_live_data(&mut self) {
        self.playlists.clear();
        self.playlists_loading = false;
        self.playlists_loaded = false;
        self.playlists_error = None;
        self.playlists_next_href = None;
        self.active_playlist_urn = None;
        self.known_playlists.clear();
        self.feed = CollectionState::default();
        self.liked_tracks = CollectionState::default();
        self.albums = CollectionState::default();
        self.following = CollectionState::default();
        self.playlist_tracks.clear();
        self.search_tracks = CollectionState::default();
        self.search_playlists = CollectionState::default();
        self.search_users = CollectionState::default();
        self.search_view = SearchView::Tracks;
        self.active_user_profile = None;
        self.user_profile_tracks = CollectionState::default();
        self.user_profile_playlists = CollectionState::default();
        self.user_profile_view = UserProfileView::Tracks;
        self.search_cache.clear();
        self.selected_playlist = 0;
        self.selected_content = 0;
        self.add_to_playlist_modal = None;
        self.queue = QueueState::default();
        self.playback_plan = PlaybackPlanState::default();
        self.player = PlayerState {
            status: PlaybackStatus::Stopped,
            volume_percent: 50.0,
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

    fn apply_playlists_page(&mut self, page: Page<SoundcloudPlaylist>, append: bool) {
        self.playlists_loading = false;
        self.playlists_loaded = true;
        self.playlists_error = None;
        self.playlists_next_href = page.next_href.clone();

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

        if append {
            self.playlists.extend(mapped);
        } else {
            self.playlists = mapped;
        }

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

    fn apply_search_results(&mut self, results: SearchResults) {
        self.search_tracks.apply_page(results.tracks, false);
        for playlist in &results.playlists.items {
            self.remember_playlist(playlist.clone());
        }
        self.search_playlists.apply_page(results.playlists, false);
        self.search_users.apply_page(results.users, false);
    }

    fn remember_playlist(&mut self, playlist: SoundcloudPlaylist) {
        self.known_playlists.insert(playlist.urn.clone(), playlist);
    }

    fn active_playlist(&self) -> Option<&SoundcloudPlaylist> {
        self.active_playlist_urn
            .as_ref()
            .and_then(|urn| self.known_playlists.get(urn))
    }

    fn open_playlist(&mut self, playlist: SoundcloudPlaylist) {
        let urn = playlist.urn.clone();
        self.remember_playlist(playlist.clone());
        self.active_playlist_urn = Some(urn.clone());

        if let Some(index) = self
            .playlists
            .iter()
            .position(|sidebar| sidebar.urn.as_deref() == Some(urn.as_str()))
        {
            self.selected_playlist = index;
        }

        self.set_route(Route::Playlist);
        self.status = format!("Opened playlist {}.", playlist.title);
    }

    fn open_user_profile(&mut self, user: UserSummary) {
        self.active_user_profile = Some(user.clone());
        self.user_profile_tracks = CollectionState::default();
        self.user_profile_playlists = CollectionState::default();
        self.user_profile_view = UserProfileView::Tracks;
        self.set_route(Route::UserProfile);
        self.status = format!("Opened {}'s profile.", user.username);
    }

    fn active_user_profile_urn(&self) -> Option<&str> {
        self.active_user_profile
            .as_ref()
            .map(|user| user.urn.as_str())
    }

    fn recent_history_state_label(&self) -> String {
        if self.recent_history.entries.is_empty() {
            "Empty".to_string()
        } else {
            format!("Loaded {} local plays", self.recent_history.entries.len())
        }
    }

    fn search_subtitle(&self) -> String {
        format!(
            "Showing {} for '{}'. Tracks: {} | Playlists: {} | Users: {}",
            self.search_view.label(),
            self.search_query,
            self.search_tracks.items.len(),
            self.search_playlists.items.len(),
            self.search_users.items.len(),
        )
    }

    fn search_title(&self) -> String {
        if self.search_query.is_empty() {
            "Search".to_string()
        } else {
            format!("Search: {}", self.search_query)
        }
    }

    fn search_help_message(&self) -> String {
        let pagination = if self.search_view == SearchView::Tracks {
            "Use Ctrl+d and Ctrl+u to jump across result pages."
        } else {
            "Playlist and creator results are first-page snapshots for now."
        };

        format!(
            "Press / to refine the query and 1/2/3 to switch search tables. {} Enter opens playlists or profiles.",
            pagination
        )
    }

    fn set_search_view(&mut self, search_view: SearchView) {
        self.search_view = search_view;
        self.selected_content = 0;
        self.status = format!("Showing {} search results.", search_view.label());
    }

    fn set_user_profile_view(&mut self, user_profile_view: UserProfileView) {
        if self.user_profile_view == user_profile_view {
            return;
        }

        self.user_profile_view = user_profile_view;
        self.selected_content = 0;
        self.status = format!(
            "Showing {} for {}.",
            self.user_profile_view.label(),
            self.route_title()
        );
        self.request_route_load(false);
    }

    fn user_profile_subtitle(&self, user: &UserSummary) -> String {
        let mut segments = vec![
            format!("Followers {}", user.followers_label()),
            format!("{} tracks", user.track_count),
            format!("{} playlists", user.playlist_count),
            format!("Showing {}", self.user_profile_view.label()),
        ];

        if let Some(permalink) = user.permalink_url.as_deref() {
            segments.push(permalink.to_string());
        }

        segments.join(" | ")
    }

    fn cache_search_results(&mut self) {
        if self.search_query.trim().is_empty() {
            return;
        }

        self.search_cache
            .insert(self.search_query.clone(), SearchCache::from_state(self));
    }

    fn dismiss_help(&mut self) {
        self.show_help = false;

        if self.help_requires_acknowledgement {
            self.help_requires_acknowledgement = false;
            if self.settings.show_help_on_startup {
                self.settings.show_help_on_startup = false;
                self.queue_command(AppCommand::SaveSettings(self.settings.clone()));
            }
            self.status = "Help dismissed. You can reopen it anytime with ?.".to_string();
        } else {
            self.status = "Help closed.".to_string();
        }
    }

    fn adjust_sidebar_width(&mut self, delta: i16) {
        let next = (self.layout.sidebar_width_percent as i16 + delta).clamp(14, 40) as u16;
        self.layout.sidebar_width_percent = next;
        self.status = format!("Sidebar width set to {}%.", next);
    }

    fn adjust_primary_panel_height(&mut self, delta: i16) {
        match self.focus {
            Focus::Library => {
                let next = (self.layout.library_height as i16 + delta).clamp(4, 18) as u16;
                self.layout.library_height = next;
                self.status = format!("Library height set to {} rows.", next);
            }
            _ => {
                let next = (self.layout.playbar_height as i16 + delta).clamp(4, 12) as u16;
                self.layout.playbar_height = next;
                self.status = format!("Playbar height set to {} rows.", next);
            }
        }
    }

    fn reset_layout(&mut self) {
        self.layout = LayoutState::default();
        self.status = "Layout reset to defaults.".to_string();
    }

    fn copy_now_playing_url(&mut self) {
        let Some(track) = self.now_playing.track.as_ref() else {
            self.show_main_error(
                "Could not copy share URL",
                "Nothing is playing right now, so there is no SoundCloud URL to copy.",
            );
            return;
        };

        let Some(url) = track.permalink_url.as_deref() else {
            self.show_main_error(
                "Could not copy share URL",
                format!("No SoundCloud URL is available for {}.", track.title),
            );
            return;
        };

        let label = track.title.clone();
        let text = url.to_string();

        self.queue_command(AppCommand::CopyText {
            text,
            label: label.clone(),
        });
        self.status = format!("Copying {} URL to the clipboard...", label);
    }

    fn cycle_repeat_mode(&mut self) {
        let next_mode = match self.player.repeat_mode {
            RepeatMode::Off => RepeatMode::Track,
            RepeatMode::Track => RepeatMode::Queue,
            RepeatMode::Queue => RepeatMode::Off,
        };
        self.set_repeat_mode(next_mode);
    }

    fn reload_current_route(&mut self) {
        if self.focus == Focus::Playlists {
            self.playlists.clear();
            self.playlists_loading = false;
            self.playlists_loaded = false;
            self.playlists_error = None;
            self.playlists_next_href = None;
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

    fn record_recent_playback(&mut self) {
        let Some(track) = self.now_playing.track.clone() else {
            return;
        };

        self.recent_history
            .record(track, self.now_playing.context.clone());
        self.queue_command(AppCommand::SaveHistory(self.recent_history.clone()));
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
            Route::RecentlyPlayed => self
                .recent_history
                .entries
                .get(index)
                .cloned()
                .map(|entry| SelectedContent::Track {
                    track: entry.track,
                    context: entry.context,
                }),
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
            Route::Playlist => self
                .active_playlist_urn
                .as_ref()
                .and_then(|urn| self.playlist_tracks.get(urn))
                .and_then(|state| state.items.get(index))
                .cloned()
                .map(|track| SelectedContent::Track {
                    context: self.route_title(),
                    track,
                }),
            Route::UserProfile => match self.user_profile_view {
                UserProfileView::Tracks => {
                    self.user_profile_tracks
                        .items
                        .get(index)
                        .cloned()
                        .map(|track| SelectedContent::Track {
                            context: self.route_title(),
                            track,
                        })
                }
                UserProfileView::Playlists => self
                    .user_profile_playlists
                    .items
                    .get(index)
                    .cloned()
                    .map(SelectedContent::Playlist),
            },
            Route::Search => match self.search_view {
                SearchView::Tracks => self.search_tracks.items.get(index).cloned().map(|track| {
                    SelectedContent::Track {
                        track,
                        context: format!("Search: {}", self.search_query),
                    }
                }),
                SearchView::Playlists => self
                    .search_playlists
                    .items
                    .get(index)
                    .cloned()
                    .map(SelectedContent::Playlist),
                SearchView::Users => self
                    .search_users
                    .items
                    .get(index)
                    .cloned()
                    .map(SelectedContent::User),
            },
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
        self.focus_content_from(self.search_return_focus);
        self.route = Route::Search;
        self.search_view = SearchView::Tracks;
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

    fn delete_search_to_start(&mut self) {
        if self.search_cursor == 0 {
            return;
        }

        let chars = self.search_query.chars().collect::<Vec<_>>();
        self.search_query = chars[self.search_cursor..].iter().copied().collect();
        self.search_cursor = 0;
        self.status = "Deleted text before the cursor.".to_string();
    }

    fn delete_search_to_end(&mut self) {
        let chars = self.search_query.chars().collect::<Vec<_>>();
        if self.search_cursor >= chars.len() {
            return;
        }

        self.search_query = chars[..self.search_cursor].iter().copied().collect();
        self.status = "Deleted text after the cursor.".to_string();
    }

    fn delete_previous_word(&mut self) {
        if self.search_cursor == 0 {
            return;
        }

        let chars = self.search_query.chars().collect::<Vec<_>>();
        let mut word_start = self.search_cursor;

        while word_start > 0 && chars[word_start - 1].is_whitespace() {
            word_start -= 1;
        }
        while word_start > 0 && !chars[word_start - 1].is_whitespace() {
            word_start -= 1;
        }

        let mut updated = chars[..word_start].to_vec();
        updated.extend_from_slice(&chars[self.search_cursor..]);
        self.search_query = updated.into_iter().collect();
        self.search_cursor = word_start;
        self.status = "Deleted the previous word.".to_string();
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
                help_message: Some("Mock feed preview.".to_string()),
            },
            Route::LikedSongs => ContentView {
                title: "Liked Songs".to_string(),
                subtitle: "Mock favorites pinned for the shell.".to_string(),
                columns: ["Title", "Artist", "Collection", "Length"],
                rows: self.liked_rows.clone(),
                state_label: "Mock data".to_string(),
                empty_message: "No mock liked songs available.".to_string(),
                help_message: Some("Mock liked songs preview.".to_string()),
            },
            Route::RecentlyPlayed => ContentView {
                title: "Recently Played".to_string(),
                subtitle: "Local history will become real in a later phase.".to_string(),
                columns: ["Title", "Artist", "Last Context", "Length"],
                rows: self.recent_rows.clone(),
                state_label: "Mock data".to_string(),
                empty_message: "No mock history available.".to_string(),
                help_message: Some("Mock recently played preview.".to_string()),
            },
            Route::Albums => ContentView {
                title: "Albums".to_string(),
                subtitle: "Album-like sets surfaced as a dedicated mock view.".to_string(),
                columns: ["Album", "Creator", "Tracks", "Year"],
                rows: self.album_rows.clone(),
                state_label: "Mock data".to_string(),
                empty_message: "No mock albums available.".to_string(),
                help_message: Some("Mock albums preview.".to_string()),
            },
            Route::Following => ContentView {
                title: "Following".to_string(),
                subtitle: "Mock creator directory with a few followed accounts.".to_string(),
                columns: ["Creator", "Followers", "Spotlight", "Status"],
                rows: self.following_rows.clone(),
                state_label: "Mock data".to_string(),
                empty_message: "No mock following rows available.".to_string(),
                help_message: Some("Mock following preview.".to_string()),
            },
            Route::Playlist => {
                let playlist = &self.playlists[self.selected_playlist];
                ContentView {
                    title: playlist.title.clone(),
                    subtitle: playlist.description.clone(),
                    columns: ["Title", "Artist", "Playlist", "Length"],
                    rows: playlist.tracks.clone(),
                    state_label: "Mock data".to_string(),
                    empty_message: "No mock playlist tracks available.".to_string(),
                    help_message: Some("Mock playlist browsing preview.".to_string()),
                }
            }
            Route::UserProfile => ContentView {
                title: "Profile".to_string(),
                subtitle: "Creator stats and releases will appear here after authentication."
                    .to_string(),
                columns: ["Title", "Artist", "Access", "Length"],
                rows: Vec::new(),
                state_label: "Mock data".to_string(),
                empty_message: "No mock creator profile is available.".to_string(),
                help_message: Some("Press Enter on a creator to open their profile.".to_string()),
            },
            Route::Search => ContentView {
                title: self.search_title(),
                subtitle: format!(
                    "Showing {} for '{}'. Tracks: {} | Playlists: 0 | Users: 0",
                    self.search_view.label(),
                    self.search_query,
                    self.search_rows.len(),
                ),
                columns: ["Title", "Artist", "Collection", "Length"],
                rows: self.search_rows.clone(),
                state_label: "Mock data".to_string(),
                empty_message: "No mock search results available.".to_string(),
                help_message: Some("Press 1, 2, or 3 to switch search result tables.".to_string()),
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

fn playlist_row(playlist: &SoundcloudPlaylist) -> ContentRow {
    ContentRow {
        columns: [
            playlist.title.clone(),
            playlist.creator.clone(),
            playlist.track_count_label(),
            playlist.year_label(),
        ],
    }
}

fn user_row(user: &UserSummary) -> ContentRow {
    ContentRow {
        columns: [
            user.username.clone(),
            user.followers_label(),
            user.spotlight_label(),
            "Profile".to_string(),
        ],
    }
}

fn history_row(entry: &RecentlyPlayedEntry) -> ContentRow {
    ContentRow {
        columns: [
            entry.track.title.clone(),
            entry.track.artist.clone(),
            entry.context.clone(),
            relative_time_label(entry.played_at_epoch),
        ],
    }
}

fn relative_time_label(played_at_epoch: i64) -> String {
    let elapsed = (Utc::now().timestamp() - played_at_epoch).max(0);

    match elapsed {
        0..=59 => "just now".to_string(),
        60..=3_599 => format!("{}m ago", elapsed / 60),
        3_600..=86_399 => format!("{}h ago", elapsed / 3_600),
        86_400..=604_799 => format!("{}d ago", elapsed / 86_400),
        _ => format!("{}w ago", elapsed / 604_800),
    }
}

fn playlist_summary_subtitle(playlist: &SoundcloudPlaylist) -> String {
    if !playlist.description.trim().is_empty() {
        playlist.description.clone()
    } else {
        format!("By {} - {}", playlist.creator, playlist.track_count_label())
    }
}

fn help_row(
    description: impl Into<String>,
    event: impl Into<String>,
    context: impl Into<String>,
) -> HelpRow {
    HelpRow {
        description: description.into(),
        event: event.into(),
        context: context.into(),
    }
}
