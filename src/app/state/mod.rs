use std::{
    collections::{HashMap, VecDeque},
    ops::Deref,
    time::{Duration, Instant},
};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

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
            FeedItem, FeedOrigin, PlaylistSummary as SoundcloudPlaylist, TrackSummary, UserSummary,
        },
        paging::Page,
    },
    ui::{geometry, theme::Theme},
    visualizer::{SpectrumFrame, VisualizerCommand, VisualizerStyle},
};

mod content;
mod events;
mod helpers;
mod init;
mod interaction;
mod loading;
mod playback;

use helpers::*;

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
    pub logout_confirm_modal: Option<LogoutConfirmModal>,
    pub toast: Option<Toast>,
    pub help_scroll: usize,
    pub auth: AuthState,
    pub session: Option<AuthorizedSession>,
    pub auth_summary: String,
    pub status: String,
    pub tick_count: u64,
    pub viewport: Viewport,
    last_mouse_click: Option<MouseClickState>,
    pub loading: Option<LoadingState>,
    pub search_query: String,
    pub search_cursor: usize,
    pub search_return_focus: Focus,
    pub library_items: Vec<LibraryItem>,
    pub playlists: PlaylistSidebarState,
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
    pub visualizer: VisualizerState,
    settings: Settings,
    help_requires_acknowledgement: bool,
    content_return_focus: Focus,
    pending_commands: VecDeque<AppCommand>,
    recent_history: RecentlyPlayedStore,
    active_playlist_urn: Option<String>,
    known_playlists: HashMap<String, SoundcloudPlaylist>,
    feed: CollectionState<FeedItem>,
    feed_request: RequestTracker,
    liked_tracks: CollectionState<TrackSummary>,
    liked_tracks_request: RequestTracker,
    albums: CollectionState<SoundcloudPlaylist>,
    albums_request: RequestTracker,
    following: CollectionState<UserSummary>,
    following_request: RequestTracker,
    playlist_tracks: HashMap<String, CollectionState<TrackSummary>>,
    playlist_track_requests: HashMap<String, RequestTracker>,
    search_tracks: CollectionState<TrackSummary>,
    search_playlists: CollectionState<SoundcloudPlaylist>,
    search_users: CollectionState<UserSummary>,
    search_request: RequestTracker,
    search_view: SearchView,
    active_user_profile: Option<UserSummary>,
    user_profile_tracks: CollectionState<TrackSummary>,
    user_profile_tracks_request: RequestTracker,
    user_profile_playlists: CollectionState<SoundcloudPlaylist>,
    user_profile_playlists_request: RequestTracker,
    user_profile_view: UserProfileView,
    search_cache: HashMap<String, SearchCache>,
    playback_plan: PlaybackPlanState,
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
pub struct LogoutConfirmModal {
    pub username: Option<String>,
    pub discard_unsaved_changes: bool,
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

#[derive(Debug, Clone, PartialEq)]
pub struct VisualizerState {
    pub visible: bool,
    pub style: VisualizerStyle,
    pub capture_active: bool,
    pub spectrum: SpectrumFrame,
    pub status: String,
}

impl Default for VisualizerState {
    fn default() -> Self {
        Self {
            visible: false,
            style: VisualizerStyle::default(),
            capture_active: false,
            spectrum: SpectrumFrame::default(),
            status: "Press v to start system audio capture.".to_string(),
        }
    }
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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct MouseClickState {
    target: MouseClickTarget,
    at: Instant,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum MouseClickTarget {
    ContentRow(Route, usize),
}

const DOUBLE_CLICK_WINDOW: Duration = Duration::from_millis(400);

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

#[derive(Debug, Clone, Default)]
pub struct PlaylistSidebarState {
    items: Vec<SidebarPlaylist>,
    next_href: Option<String>,
    loading: bool,
    error: Option<String>,
    loaded: bool,
    request: RequestTracker,
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
struct RequestTracker {
    current: u64,
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
        self.state_label_with_more_available(true)
    }

    fn state_label_with_more_available(&self, show_more_available: bool) -> String {
        if self.loading {
            "Loading".to_string()
        } else if self.error.is_some() {
            "Error".to_string()
        } else if self.loaded {
            if self.items.is_empty() {
                "Empty".to_string()
            } else if show_more_available && self.next_href.is_some() {
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

impl PlaylistSidebarState {
    fn is_loading(&self) -> bool {
        self.loading
    }

    fn is_loaded(&self) -> bool {
        self.loaded
    }

    fn has_error(&self) -> bool {
        self.error.is_some()
    }

    fn next_href(&self) -> Option<&str> {
        self.next_href.as_deref()
    }

    fn start_loading(&mut self, append: bool) -> u64 {
        self.loading = true;
        self.error = None;
        if !append {
            self.next_href = None;
            self.items.clear();
        }
        self.request.issue(append)
    }

    fn apply_page(&mut self, page: Page<SidebarPlaylist>, append: bool) {
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
        self.loaded = true;
        self.error = Some(error);
    }

    fn reset(&mut self) {
        self.items.clear();
        self.loading = false;
        self.loaded = false;
        self.error = None;
        self.next_href = None;
        self.request = RequestTracker::default();
    }

    fn invalidate(&mut self) {
        self.items.clear();
        self.loading = false;
        self.loaded = false;
        self.error = None;
        self.next_href = None;
        self.request.invalidate();
    }

    fn matches_request(&self, request_id: u64) -> bool {
        self.request.matches(request_id)
    }

    fn title(&self) -> String {
        if self.loading {
            "Playlists (loading...)".to_string()
        } else if self.error.is_some() {
            "Playlists (error)".to_string()
        } else if self.loaded && self.items.is_empty() {
            "Playlists (empty)".to_string()
        } else if self.next_href.is_some() {
            format!("Playlists ({}, more available)", self.items.len())
        } else {
            format!("Playlists ({})", self.items.len())
        }
    }

    fn placeholder(&self) -> Option<String> {
        if self.loading && self.items.is_empty() {
            Some("Loading playlists...".to_string())
        } else if self.error.is_some() {
            Some("Could not load playlists. Press F5 to retry.".to_string())
        } else if self.loaded && self.items.is_empty() {
            Some("No playlists are available for this account yet.".to_string())
        } else {
            None
        }
    }
}

impl Deref for PlaylistSidebarState {
    type Target = [SidebarPlaylist];

    fn deref(&self) -> &Self::Target {
        self.items.as_slice()
    }
}

impl RequestTracker {
    fn issue(&mut self, append: bool) -> u64 {
        if !append || self.current == 0 {
            self.current = self.current.saturating_add(1).max(1);
        }

        self.current
    }

    fn invalidate(&mut self) {
        self.current = self.current.saturating_add(1).max(1);
    }

    fn matches(self, request_id: u64) -> bool {
        self.current == request_id
    }
}

#[cfg(test)]
mod tests {
    use super::CollectionState;

    #[test]
    fn state_label_hides_more_available_when_disabled() {
        let state = CollectionState {
            items: vec![1, 2, 3],
            next_href: Some("https://api.soundcloud.com/next".to_string()),
            loading: false,
            error: None,
            loaded: true,
        };

        assert_eq!(state.state_label(), "Loaded 3 items (more available)");
        assert_eq!(
            state.state_label_with_more_available(false),
            "Loaded 3 items"
        );
    }
}
