use crate::{
    config::{
        credentials::Credentials, history::RecentlyPlayedStore, settings::Settings,
        tokens::TokenStore,
    },
    player::command::PlayerCommand,
    soundcloud::auth::{AuthorizationRequest, AuthorizedSession},
    soundcloud::models::{PlaylistSummary, TrackSummary},
    visualizer::VisualizerCommand,
};

#[derive(Debug, Clone)]
pub enum AppCommand {
    OpenUrl(String),
    SaveCredentials(AuthorizationRequest),
    ValidateSavedSession {
        credentials: Credentials,
        tokens: TokenStore,
    },
    WaitForOAuthCallback(AuthorizationRequest),
    ExchangeAuthorizationCode {
        request: AuthorizationRequest,
        callback_input: String,
    },
    Logout,
    SaveSettings(Settings),
    SetWindowTitle(String),
    SaveHistory(RecentlyPlayedStore),
    LoadFeed {
        session: AuthorizedSession,
        request_id: u64,
        next_href: Option<String>,
        append: bool,
    },
    LoadLikedSongs {
        session: AuthorizedSession,
        request_id: u64,
        next_href: Option<String>,
        append: bool,
    },
    LoadAlbums {
        session: AuthorizedSession,
        request_id: u64,
        next_href: Option<String>,
        append: bool,
    },
    LoadFollowing {
        session: AuthorizedSession,
        request_id: u64,
        next_href: Option<String>,
        append: bool,
    },
    LoadPlaylists {
        session: AuthorizedSession,
        request_id: u64,
        next_href: Option<String>,
        append: bool,
    },
    LoadPlaylistTracks {
        session: AuthorizedSession,
        request_id: u64,
        playlist_urn: String,
        next_href: Option<String>,
        append: bool,
    },
    LoadUserTracks {
        session: AuthorizedSession,
        request_id: u64,
        user_urn: String,
        next_href: Option<String>,
        append: bool,
    },
    LoadUserPlaylists {
        session: AuthorizedSession,
        request_id: u64,
        user_urn: String,
        next_href: Option<String>,
        append: bool,
    },
    SearchAll {
        session: AuthorizedSession,
        request_id: u64,
        query: String,
    },
    SearchTracksPage {
        session: AuthorizedSession,
        request_id: u64,
        query: String,
        next_href: String,
    },
    LikeTrack {
        session: AuthorizedSession,
        track: TrackSummary,
    },
    AddTrackToPlaylist {
        session: AuthorizedSession,
        track: TrackSummary,
        playlist: PlaylistSummary,
    },
    CopyText {
        text: String,
        label: String,
    },
    LoadCoverArt {
        url: String,
    },
    PlayTrack {
        session: AuthorizedSession,
        track: TrackSummary,
    },
    ControlPlayback(PlayerCommand),
    ControlVisualizer(VisualizerCommand),
}
