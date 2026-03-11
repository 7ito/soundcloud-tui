use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, Instant},
};

use chrono::Utc;
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
            FeedItem, FeedOrigin, PlaylistSummary as SoundcloudPlaylist, SearchResults,
            TrackSummary, UserSummary,
        },
        paging::Page,
    },
    ui::{geometry, theme::Theme, widgets::pane_inner},
    visualizer::{SpectrumFrame, VisualizerCommand, VisualizerStyle},
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
    last_mouse_click: Option<MouseClickState>,
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
    pub visualizer: VisualizerState,
    settings: Settings,
    help_requires_acknowledgement: bool,
    content_return_focus: Focus,
    pending_commands: VecDeque<AppCommand>,
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

include!("init.rs");
include!("events.rs");
include!("content.rs");
include!("playback.rs");
include!("loading.rs");

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
include!("interaction.rs");

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

fn rect_contains(rect: ratatui::layout::Rect, column: u16, row: u16) -> bool {
    column >= rect.x
        && column < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

fn mouse_scroll_delta(kind: MouseEventKind) -> Option<isize> {
    match kind {
        MouseEventKind::ScrollDown => Some(1),
        MouseEventKind::ScrollUp => Some(-1),
        _ => None,
    }
}

fn block_list_index_at_row(
    area: ratatui::layout::Rect,
    column: u16,
    row: u16,
    len: usize,
    selected: usize,
) -> Option<usize> {
    row_index_at(pane_inner(area), column, row, len, selected, 0)
}

fn plain_list_index_at_row(
    area: ratatui::layout::Rect,
    column: u16,
    row: u16,
    len: usize,
    selected: usize,
) -> Option<usize> {
    row_index_at(area, column, row, len, selected, 0)
}

fn table_index_at_row(
    area: ratatui::layout::Rect,
    column: u16,
    row: u16,
    len: usize,
    selected: usize,
) -> Option<usize> {
    row_index_at(area, column, row, len, selected, 1)
}

fn row_index_at(
    area: ratatui::layout::Rect,
    column: u16,
    row: u16,
    len: usize,
    selected: usize,
    header_rows: u16,
) -> Option<usize> {
    if len == 0 {
        return None;
    }

    if column < area.x || column >= area.x.saturating_add(area.width) {
        return None;
    }

    let start_row = area.y.saturating_add(header_rows);
    if row < start_row || row >= area.y.saturating_add(area.height) {
        return None;
    }

    let visible_rows = area.height.saturating_sub(header_rows) as usize;
    if visible_rows == 0 {
        return None;
    }

    let start_index = selected
        .min(len.saturating_sub(1))
        .saturating_sub(visible_rows.saturating_sub(1));
    let index = start_index + row.saturating_sub(start_row) as usize;

    (index < len).then_some(index)
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
